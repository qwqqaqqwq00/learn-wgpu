struct Uniforms {
    model_mat: mat4x4<f32>,
    view_proj_mat: mat4x4<f32>,
    norm_mat: mat4x4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms: Uniforms;


struct Output {
    @builtin(position) position: vec4<f32>,
    // @location(0) v_position: vec4<f32>,
    // @location(1) tex_coord: vec2<f32>,
    // @location(2) v_normal: vec4<f32>,
    // @location(0) color: vec4<f32>,
};

struct FragUniforms {
    light_pos: vec4<f32>,
    eye_pos: vec4<f32>,
};

@binding(1) @group(0) var<uniform> frag_uniform: FragUniforms;

@vertex
fn vs_main(@location(0) pos: vec4<f32>) -> Output {
    var output: Output;
    let m_pos: vec4<f32> = pos * 0.25 + frag_uniform.light_pos;
    // output.v_position = m_pos;
    
    
    // output.v_normal = uniforms.norm_mat * norm;
    // output.tex_coord = tex_coord;
    output.position = uniforms.view_proj_mat  * m_pos;
    // ourput.color = 
    return output;
}


struct LightUniforms {
    // color: vec4<f32>,
    specular_color: vec4<f32>,
    ambient: f32,
    diffuse: f32,
    specular_intensity: f32,
    specular_shininess: f32,
};

@binding(2) @group(0) var<uniform> light_uniform: LightUniforms;

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    
    return light_uniform.specular_color;
}