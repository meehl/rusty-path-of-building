use crate::{
    lua::ContextSocket,
    renderer::textures::{TextureHandle, TextureOptions},
};
use mlua::{Lua, MultiValue, Result as LuaResult, UserData};

pub fn new_image_handle(_: &Lua, _: ()) -> LuaResult<ImageHandle> {
    Ok(ImageHandle::Unloaded)
}

#[derive(Clone)]
pub enum ImageHandle {
    Loaded(TextureHandle),
    Unloaded,
}

impl UserData for ImageHandle {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Load", load);

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

fn load(
    lua: &Lua,
    handle: &mut ImageHandle,
    (image_path, flags): (String, MultiValue),
) -> LuaResult<()> {
    let socket = lua.app_data_ref::<&'static ContextSocket>().unwrap();
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

    match handle {
        // replace image data if already allocated
        ImageHandle::Loaded(texture_handle) => {
            // in case of error, stay loaded with current texture
            let _ = socket.texture_manager().update_texture(
                texture_handle.id(),
                image_path,
                options,
                is_async,
            );
        }
        // create new texture handle
        ImageHandle::Unloaded => {
            if let Ok(tex_handle) = socket
                .texture_manager()
                .load_texture(image_path, options, is_async)
            {
                *handle = ImageHandle::Loaded(tex_handle);
            }
        }
    }
    Ok(())
}
