use std::{cell::RefCell, collections::hash_map::Entry, path::Path, rc::Rc, sync::mpsc};

use ahash::HashMap;
use anyhow::bail;

use crate::{
    color::Srgba,
    dpi::PhysicalSize,
    renderer::image::{ImageData, ImageDelta, MipStrategy, load_image_file},
    worker_pool::WorkerPool,
};

pub type TextureId = u64;

pub struct TextureHandle {
    tex_mngr: Rc<RefCell<TextureRegistry>>,
    id: TextureId,
}

impl TextureHandle {
    fn new(tex_mngr: Rc<RefCell<TextureRegistry>>, id: TextureId) -> Self {
        Self { tex_mngr, id }
    }

    pub fn id(&self) -> TextureId {
        self.id
    }

    pub fn size(&self) -> Option<PhysicalSize<u32>> {
        match self
            .tex_mngr
            .borrow()
            .get_meta_data(self.id)
            .expect("Texture exists because we hold a handle")
            .state
        {
            TextureState::AsyncLoading => None,
            TextureState::Loaded(shape) => Some(shape.size),
        }
    }

    pub fn is_loading(&self) -> bool {
        match self
            .tex_mngr
            .borrow()
            .get_meta_data(self.id)
            .expect("Texture exists because we hold a handle")
            .state
        {
            TextureState::AsyncLoading => true,
            TextureState::Loaded(_) => false,
        }
    }
}

impl Drop for TextureHandle {
    fn drop(&mut self) {
        self.tex_mngr.borrow_mut().free(self.id);
    }
}

impl Clone for TextureHandle {
    fn clone(&self) -> Self {
        self.tex_mngr.borrow_mut().retain(self.id);
        Self {
            tex_mngr: Rc::clone(&self.tex_mngr),
            id: self.id,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TexturesDelta {
    pub update: Vec<(TextureId, ImageDelta)>,
    pub free: Vec<TextureId>,
}

impl TexturesDelta {
    pub fn is_empty(&self) -> bool {
        self.update.is_empty() && self.free.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextureShape {
    size: PhysicalSize<u32>,
    format: wgpu::TextureFormat,
    array_layers: u32,
    mip_level_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextureState {
    /// Texture is being loaded asynchronously
    AsyncLoading,
    Loaded(TextureShape),
}

/// Metadata about an allocated texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureMetaData {
    name: String,
    options: TextureOptions,
    state: TextureState,
    /// Texture is freed when this reaches zero
    retain_count: usize,
}

#[derive(Default)]
struct TextureRegistry {
    next_id: u64,
    meta_data: HashMap<TextureId, TextureMetaData>,
    delta: TexturesDelta,
}

impl TextureRegistry {
    /// Allocates a new Texture.
    pub fn alloc(
        &mut self,
        name: String,
        image: ImageData,
        options: TextureOptions,
        generate_mipmaps: bool,
    ) -> TextureId {
        let id = self.next_id;
        self.next_id += 1;

        let mip_strategy = MipStrategy::resolve(&image, generate_mipmaps);

        self.meta_data.insert(
            id,
            TextureMetaData {
                name,
                retain_count: 1,
                options,
                state: TextureState::Loaded(TextureShape {
                    size: PhysicalSize::new(image.width, image.height),
                    format: image.format,
                    array_layers: image.array_layers,
                    mip_level_count: mip_strategy.resolve_mip_level_count(&image),
                }),
            },
        );

        self.delta
            .update
            .push((id, ImageDelta::full(image, options, mip_strategy)));

        id
    }

    /// Reserves a new `TextureId` for later assignment.
    pub fn reserve(&mut self, name: String, options: TextureOptions) -> TextureId {
        let id = self.next_id;
        self.next_id += 1;

        self.meta_data.insert(
            id,
            TextureMetaData {
                name,
                retain_count: 1,
                options,
                state: TextureState::AsyncLoading,
            },
        );

        id
    }

    /// Assigns a new image to an existing texture.
    pub fn set(&mut self, id: TextureId, delta: ImageDelta) {
        let Some(meta_data) = self.meta_data.get_mut(&id) else {
            // the handle may have been dropped while the async load was in flight.
            // just discard the result.
            log::debug!("Discarding load result for freed texture {id:?}");
            return;
        };

        match &delta {
            ImageDelta::Full {
                image,
                mip_strategy,
                ..
            } => {
                meta_data.state = TextureState::Loaded(TextureShape {
                    size: PhysicalSize::new(image.width, image.height),
                    format: image.format,
                    array_layers: image.array_layers,
                    mip_level_count: mip_strategy.resolve_mip_level_count(image),
                });
                // discard all old enqueued deltas since we're doing a full update
                self.delta.update.retain(|(x, _)| x != &id);
            }
            ImageDelta::Partial { image, .. } => {
                let TextureState::Loaded(shape) = &meta_data.state else {
                    panic!("partial update for texture {id:?} which hasn't finished loading!");
                };

                assert_eq!(
                    shape.format, image.format,
                    "format of partial update must match format of existing texture {id:?}"
                );

                assert_eq!(
                    shape.mip_level_count, 1,
                    "partial updates are not supported for textures with mipmaps (texture {id:?})"
                );
            }
        }
        self.delta.update.push((id, delta));
    }

    /// Frees an existing texture.
    pub fn free(&mut self, id: TextureId) {
        if let Entry::Occupied(mut entry) = self.meta_data.entry(id) {
            let meta = entry.get_mut();
            debug_assert!(
                meta.retain_count > 0,
                "Tried freeing texture {id:?} with retain_count == 0"
            );
            meta.retain_count = meta.retain_count.saturating_sub(1);
            if meta.retain_count == 0 {
                entry.remove();
                self.delta.free.push(id);
            }
        } else {
            debug_assert!(false, "Tried freeing texture {id:?} which is not allocated");
        }
    }

    /// Increase the retain-count of the given texture.
    ///
    /// [`Self::free`] must be called an additional time for each time [`Self::retain`] is called,
    pub fn retain(&mut self, id: TextureId) {
        if let Some(meta) = self.meta_data.get_mut(&id) {
            meta.retain_count += 1;
        } else {
            debug_assert!(
                false,
                "Tried retaining texture {id:?} which is not allocated",
            );
        }
    }

    /// Get metadata about a specific texture.
    pub fn get_meta_data(&self, id: TextureId) -> Option<&TextureMetaData> {
        self.meta_data.get(&id)
    }

    /// Take and reset changes since last frame.
    pub fn take_delta(&mut self) -> TexturesDelta {
        std::mem::take(&mut self.delta)
    }
}

enum LoadResult {
    Loaded {
        id: TextureId,
        image: ImageData,
        options: TextureOptions,
        generate_mipmaps: bool,
    },
}

pub struct TextureManager {
    manager: Rc<RefCell<TextureRegistry>>,
    worker_pool: WorkerPool,
    // channel for handling async image loads
    results_tx: mpsc::Sender<LoadResult>,
    results_rx: mpsc::Receiver<LoadResult>,
}

impl TextureManager {
    pub fn new() -> Self {
        let mut manager = TextureRegistry::default();

        // allocate default texture (id: 0) for font atlas
        manager.alloc(
            "font_atlas_texture".into(),
            ImageData::from_solid_color([1, 1], Srgba::WHITE),
            TextureOptions::default(),
            false,
        );

        let (tx, rx) = mpsc::channel();

        Self {
            manager: Rc::new(RefCell::new(manager)),
            worker_pool: WorkerPool::new(4),
            results_tx: tx,
            results_rx: rx,
        }
    }

    #[inline]
    pub fn update_font_texture(&self, delta: Vec<ImageDelta>) {
        delta.into_iter().for_each(|d| {
            self.manager.borrow_mut().set(TextureId::default(), d);
        });
    }

    #[inline]
    pub fn take_delta(&self) -> TexturesDelta {
        self.apply_pending_loads();
        self.manager.borrow_mut().take_delta()
    }

    pub fn load_texture(
        &self,
        image_path: String,
        options: TextureOptions,
        is_async: bool,
        generate_mipmaps: bool,
    ) -> anyhow::Result<TextureHandle> {
        profiling::scope!("load_texture");

        if is_async {
            let id = self
                .manager
                .borrow_mut()
                .reserve(image_path.clone(), options);

            let tx = self.results_tx.clone();
            self.worker_pool
                .execute(move || match load_image_file(Path::new(&image_path)) {
                    Ok(image) => {
                        let _ = tx.send(LoadResult::Loaded {
                            id,
                            image,
                            options,
                            generate_mipmaps,
                        });
                    }
                    Err(e) => log::warn!("Unable to load image from {image_path}: {e}"),
                });

            Ok(TextureHandle::new(Rc::clone(&self.manager), id))
        } else {
            match load_image_file(Path::new(&image_path)) {
                Ok(image) => {
                    let id = self.manager.borrow_mut().alloc(
                        image_path,
                        image,
                        options,
                        generate_mipmaps,
                    );
                    Ok(TextureHandle::new(Rc::clone(&self.manager), id))
                }
                Err(e) => {
                    log::warn!("Unable to load image from {image_path}: {e}");
                    bail!(e);
                }
            }
        }
    }

    pub fn update_texture(
        &self,
        texture_id: TextureId,
        image_path: String,
        options: TextureOptions,
        is_async: bool,
        generate_mipmaps: bool,
    ) -> anyhow::Result<()> {
        if is_async {
            let tx = self.results_tx.clone();
            self.worker_pool
                .execute(move || match load_image_file(Path::new(&image_path)) {
                    Ok(image) => {
                        let _ = tx.send(LoadResult::Loaded {
                            id: texture_id,
                            image,
                            options,
                            generate_mipmaps,
                        });
                    }
                    Err(e) => log::warn!("Unable to load image from {image_path}: {e}"),
                });
        } else {
            match load_image_file(Path::new(&image_path)) {
                Ok(image) => {
                    let mip_strategy = MipStrategy::resolve(&image, generate_mipmaps);
                    self.manager
                        .borrow_mut()
                        .set(texture_id, ImageDelta::full(image, options, mip_strategy));
                }
                Err(e) => {
                    log::warn!("Unable to load image from {image_path}: {e}");
                    bail!(e);
                }
            }
        }

        Ok(())
    }

    /// Drains completed async image loads and assigns them to textures
    fn apply_pending_loads(&self) {
        let mut manager = self.manager.borrow_mut();
        while let Ok(LoadResult::Loaded {
            id,
            image,
            options,
            generate_mipmaps,
        }) = self.results_rx.try_recv()
        {
            let mip_strategy = MipStrategy::resolve(&image, generate_mipmaps);
            manager.set(id, ImageDelta::full(image, options, mip_strategy));
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextureOptions {
    pub magnification: wgpu::FilterMode,
    pub minification: wgpu::FilterMode,
    pub wrap_mode: wgpu::AddressMode,
    pub mipmap_mode: wgpu::MipmapFilterMode,
}

impl TextureOptions {
    pub const LINEAR_REPEAT: Self = Self {
        magnification: wgpu::FilterMode::Linear,
        minification: wgpu::FilterMode::Linear,
        wrap_mode: wgpu::AddressMode::Repeat,
        mipmap_mode: wgpu::MipmapFilterMode::Linear,
    };

    pub const LINEAR: Self = Self {
        magnification: wgpu::FilterMode::Linear,
        minification: wgpu::FilterMode::Linear,
        wrap_mode: wgpu::AddressMode::ClampToEdge,
        mipmap_mode: wgpu::MipmapFilterMode::Linear,
    };
}

impl Default for TextureOptions {
    fn default() -> Self {
        Self::LINEAR
    }
}
