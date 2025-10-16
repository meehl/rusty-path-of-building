use mlua::{MultiValue, UserData};

use crate::{
    context::CONTEXT,
    renderer::textures::{TextureHandle, TextureOptions},
};

#[derive(Clone)]
pub enum ImageHandle {
    Loaded(TextureHandle),
    Unloaded,
}

impl UserData for ImageHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut(
            "Load",
            |_, this, (image_path, flags): (String, MultiValue)| {
                let mut is_async = false;
                let mut options = TextureOptions::LINEAR_REPEAT;

                for flag in flags.iter() {
                    if let Some(flag) = flag.as_string() {
                        match flag.to_string_lossy().as_str() {
                            "CLAMP" => options.wrap_mode = wgpu::AddressMode::ClampToEdge,
                            "NEAREST" => options.magnification = wgpu::FilterMode::Nearest,
                            "ASYNC" => is_async = true,
                            "MIPMAP" => options.generate_mipmaps = true,
                            _ => {}
                        }
                    }
                }

                match this {
                    // replace image data if already allocated
                    ImageHandle::Loaded(texture_handle) => {
                        CONTEXT.with_borrow(|ctx| {
                            // in case of error, stay loaded with current texture
                            let _ = ctx.texture_manager.update_texture(
                                texture_handle.id(),
                                image_path,
                                options,
                                is_async,
                            );
                        });
                    }
                    // create new texture handle
                    ImageHandle::Unloaded => {
                        CONTEXT.with_borrow(|ctx| {
                            if let Ok(handle) = ctx
                                .texture_manager
                                .load_texture(image_path, options, is_async)
                            {
                                *this = ImageHandle::Loaded(handle);
                            }
                        });
                    }
                }
                Ok(())
            },
        );

        methods.add_method_mut("Unload", |_, this, ()| {
            match this {
                ImageHandle::Loaded(_) => {
                    // dropping the handle frees the texture
                    *this = ImageHandle::Unloaded;
                }
                ImageHandle::Unloaded => {}
            }
            Ok(())
        });

        methods.add_method("IsValid", |_, this, ()| {
            Ok(matches!(this, ImageHandle::Loaded(_)))
        });

        methods.add_method("IsLoading", |_, this, ()| match &this {
            ImageHandle::Loaded(texture_handle) => {
                let size = texture_handle.size();
                Ok(size[0] == 0)
            }
            ImageHandle::Unloaded => Ok(true),
        });

        methods.add_method("ImageSize", |_, this, ()| match &this {
            ImageHandle::Loaded(texture_handle) => {
                let size = texture_handle.size();
                Ok((size[0], size[1]))
            }
            ImageHandle::Unloaded => Ok((0, 0)),
        });
    }
}
