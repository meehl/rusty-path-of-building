// Vertex shader bindings
struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>, // linear
    @location(2) layer_idx: u32,
    @builtin(position) position: vec4<f32>,
};

struct Globals {
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> r_globals: Globals;

// [u8; 4] as u32 -> [f32; 4]
fn unpack_color(color: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(color & 255u),
        f32((color >> 8u) & 255u),
        f32((color >> 16u) & 255u),
        f32((color >> 24u) & 255u),
    ) / 255.0;
}

fn position_from_screen(screen_pos: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(
        2.0 * screen_pos.x / r_globals.screen_size.x - 1.0,
        1.0 - 2.0 * screen_pos.y / r_globals.screen_size.y,
        0.0,
        1.0,
    );
}

@vertex
fn vs_main(
    @location(0) a_pos: vec2<f32>,
    @location(1) a_tex_coord: vec2<f32>,
    @location(2) a_color: u32, // non-linear
    @location(3) a_layer_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;
    out.layer_idx = a_layer_idx;
    out.color = unpack_color(a_color);
    out.position = position_from_screen(a_pos);
    return out;
}

@group(1) @binding(0) var r_tex_color: texture_2d_array<f32>;
@group(1) @binding(1) var r_tex_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // NOTE: PoB incorrectly performs mixing and blending in sRGB space.
    // To get a similar visual result, we need to do the same.
    // Vertex colors, texture samples, and the output color are all in sRGB.
    // Texture formats and output surface formats are selected such that no automatic
    // conversion between linear <-> sRGB is performed.
    let tex_color = textureSample(r_tex_color, r_tex_sampler, in.tex_coord, in.layer_idx);
    var out_color = in.color * tex_color;
    return out_color;
}
