use std::{
    collections::hash_map::Entry,
    path::Path,
    sync::{Arc, RwLock},
};

use ahash::HashMap;
use anyhow::bail;

use crate::{
    color::Srgba,
    renderer::image::{ImageData, ImageDelta, load_image_file},
    worker_pool::WorkerPool,
};

pub type TextureId = u64;

pub struct TextureHandle {
    tex_mngr: Arc<RwLock<TextureManager>>,
    id: TextureId,
}

impl TextureHandle {
    pub fn new(tex_mngr: Arc<RwLock<TextureManager>>, id: TextureId) -> Self {
        Self { tex_mngr, id }
    }

    pub fn id(&self) -> TextureId {
        self.id
    }

    pub fn size(&self) -> [usize; 2] {
        self.tex_mngr
            .read()
            .unwrap()
            .get_meta_data(self.id)
            .map_or([0, 0], |tex| tex.size)
    }
}

impl Drop for TextureHandle {
    fn drop(&mut self) {
        self.tex_mngr.write().unwrap().free(self.id);
    }
}

impl Clone for TextureHandle {
    fn clone(&self) -> Self {
        self.tex_mngr.write().unwrap().retain(self.id);
        Self {
            tex_mngr: Arc::clone(&self.tex_mngr),
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

/// Metadata about an allocated texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureMetaData {
    pub name: String,
    pub size: [usize; 2],
    /// Texture is freed when this reaches zero
    retain_count: usize,
    pub options: TextureOptions,
}

#[derive(Default)]
pub struct TextureManager {
    next_id: u64,
    meta_data: HashMap<TextureId, TextureMetaData>,
    delta: TexturesDelta,
}

impl TextureManager {
    /// Allocates a new Texture.
    pub fn alloc(&mut self, name: String, image: ImageData, options: TextureOptions) -> TextureId {
        let id = self.next_id;
        self.next_id += 1;

        self.meta_data.entry(id).or_insert_with(|| TextureMetaData {
            name,
            size: [image.width as usize, image.height as usize],
            retain_count: 1,
            options,
        });

        self.delta
            .update
            .push((id, ImageDelta::new(image, options)));

        id
    }

    /// Reserves a new TextureId for later assignment.
    pub fn reserve(&mut self, name: String, options: TextureOptions) -> TextureId {
        let id = self.next_id;
        self.next_id += 1;

        self.meta_data.entry(id).or_insert_with(|| TextureMetaData {
            name,
            size: [0, 0],
            retain_count: 1,
            options,
        });

        id
    }

    /// Assigns a new image to an existing texture.
    pub fn set(&mut self, id: TextureId, delta: ImageDelta) {
        if let Some(meta_data) = self.meta_data.get_mut(&id) {
            meta_data.size = [delta.image.width as usize, delta.image.height as usize];
            // discard all old enqueued deltas
            self.delta.update.retain(|(x, _)| x != &id);
            self.delta.update.push((id, delta));
        } else {
            debug_assert!(false, "Tried setting texture {id:?} which is not allocated");
        }
    }

    /// Frees an existing texture.
    pub fn free(&mut self, id: TextureId) {
        if let Entry::Occupied(mut entry) = self.meta_data.entry(id) {
            let meta = entry.get_mut();
            meta.retain_count -= 1;
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

pub struct WrappedTextureManager {
    manager: Arc<RwLock<TextureManager>>,
    worker_pool: WorkerPool,
}

impl WrappedTextureManager {
    pub fn new() -> Self {
        let manager = Arc::new(RwLock::new(TextureManager::default()));

        // allocate default texture (id: 0) for font atlas
        manager.write().unwrap().alloc(
            "font_atlas_texture".into(),
            ImageData::from_solid_color([0, 0], Srgba::TRANSPARENT),
            TextureOptions::default(),
        );

        Self {
            manager,
            worker_pool: WorkerPool::new(4),
        }
    }

    #[inline]
    pub fn update_font_texture(&self, delta: ImageDelta) {
        self.manager
            .write()
            .unwrap()
            .set(TextureId::default(), delta);
    }

    #[inline]
    pub fn take_delta(&self) -> TexturesDelta {
        self.manager.write().unwrap().take_delta()
    }

    pub fn load_texture(
        &self,
        image_path: String,
        options: TextureOptions,
        is_async: bool,
    ) -> anyhow::Result<TextureHandle> {
        let manager = Arc::clone(&self.manager);

        let handle = if is_async {
            let id = manager
                .write()
                .unwrap()
                .reserve(image_path.clone(), options);

            // load image in background worker
            let mngr_clone = Arc::clone(&manager);
            self.worker_pool
                .execute(move || match load_image_file(Path::new(&image_path)) {
                    Ok(image) => {
                        mngr_clone
                            .write()
                            .unwrap()
                            .set(id, ImageDelta::new(image, options));
                    }
                    Err(e) => log::warn!("Unable to load image fron {}: {}", &image_path, e),
                });

            TextureHandle::new(manager, id)
        } else {
            match load_image_file(Path::new(&image_path)) {
                Ok(image) => {
                    let id = manager.write().unwrap().alloc(image_path, image, options);
                    TextureHandle::new(manager, id)
                }
                Err(e) => {
                    log::warn!("Unable to load image fron {}: {}", &image_path, e);
                    bail!(e);
                }
            }
        };

        Ok(handle)
    }

    pub fn update_texture(
        &self,
        texture_id: TextureId,
        image_path: String,
        options: TextureOptions,
        is_async: bool,
    ) -> anyhow::Result<()> {
        if is_async {
            let mngr_clone = Arc::clone(&self.manager);
            self.worker_pool
                .execute(move || match load_image_file(Path::new(&image_path)) {
                    Ok(image) => {
                        mngr_clone
                            .write()
                            .unwrap()
                            .set(texture_id, ImageDelta::new(image, options));
                    }
                    Err(e) => log::warn!("Unable to load image fron {}: {}", &image_path, e),
                });
        } else {
            match load_image_file(Path::new(&image_path)) {
                Ok(image) => {
                    self.manager
                        .write()
                        .unwrap()
                        .set(texture_id, ImageDelta::new(image, options));
                }
                Err(e) => {
                    log::warn!("Unable to load image fron {}: {}", &image_path, e);
                    bail!(e);
                }
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TextureOptions {
    pub magnification: wgpu::FilterMode,
    pub minification: wgpu::FilterMode,
    pub wrap_mode: wgpu::AddressMode,
    pub mipmap_mode: wgpu::FilterMode,
    pub generate_mipmaps: bool,
}

impl TextureOptions {
    pub const LINEAR_REPEAT: Self = Self {
        magnification: wgpu::FilterMode::Linear,
        minification: wgpu::FilterMode::Linear,
        wrap_mode: wgpu::AddressMode::Repeat,
        mipmap_mode: wgpu::FilterMode::Linear,
        generate_mipmaps: false,
    };

    pub const LINEAR: Self = Self {
        magnification: wgpu::FilterMode::Linear,
        minification: wgpu::FilterMode::Linear,
        wrap_mode: wgpu::AddressMode::ClampToEdge,
        mipmap_mode: wgpu::FilterMode::Linear,
        generate_mipmaps: false,
    };
}

impl Default for TextureOptions {
    fn default() -> Self {
        Self::LINEAR
    }
}

impl std::hash::Hash for TextureOptions {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.magnification.hash(state);
        self.minification.hash(state);
        self.wrap_mode.hash(state);
        self.mipmap_mode.hash(state);
    }
}
