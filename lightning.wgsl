struct Uniforms {
    model_mat: mat4x4<f32>,
    view_proj_mat: mat4x4<f32>,
    norm_mat: mat4x4<f32>,
};

@binding(0) @group(0) var<uniform> uniforms: Uniforms;

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
};

struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) v_position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    // @location(2) v_normal: vec4<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec4<f32>, @location(1) tex_coord: vec2<f32>, @location(2) norm: vec4<f32>, instance: InstanceInput) -> Output {
    var output: Output;
    let model_mat = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let m_pos: vec4<f32> = uniforms.model_mat * pos;
    output.v_position = m_pos;
    
    
    // output.v_normal = uniforms.norm_mat * norm;
    output.tex_coord = tex_coord;
    output.position = uniforms.view_proj_mat * model_mat * pos;
    return output;
}

struct FragUniforms {
    light_pos: vec4<f32>,
    eye_pos: vec4<f32>,
};

@binding(1) @group(0) var<uniform> frag_uniform: FragUniforms;

struct LightUniforms {
    // color: vec4<f32>,
    specular_color: vec4<f32>,
    ambient: f32,
    diffuse: f32,
    specular_intensity: f32,
    specular_shininess: f32,
};

@binding(2) @group(0) var<uniform> light_uniform: LightUniforms;

@binding(0) @group(1) var t_diffuse: texture_2d<f32>;
@binding(1) @group(1) var s_diffuse: sampler;
@binding(2) @group(1) var t_normal: texture_2d<f32>;
@binding(3) @group(1) var s_normal: sampler;

@fragment
fn fs_main(@location(0) v_pos: vec4<f32>, @location(1) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    
    let v_normal = textureSample(t_normal, s_normal, tex_coord);
    // 法线
    let N: vec3<f32> = normalize(v_normal.xyz);
    // 光源入射
    let L: vec3<f32> = normalize(frag_uniform.light_pos.xyz - v_pos.xyz);
    // 相机出射
    let V: vec3<f32> = normalize(frag_uniform.eye_pos.xyz - v_pos.xyz);
    // 对角线
    let H = normalize(L + V);


    let diffuse: f32 = light_uniform.diffuse * max(dot(N, L), 0.0);
    let specular: f32 = light_uniform.specular_intensity * pow(max(dot(N, H), 0.0), light_uniform.specular_shininess);
    let ambient: f32 = light_uniform.ambient;
    let color: vec3<f32> = light_uniform.specular_color.xyz * (specular + ambient + diffuse);
    let obj_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, tex_coord);
    return vec4<f32>(color * obj_color.xyz, obj_color.a);
    // return  light_uniform.specular_color * (specular + ambient + diffuse) * textureSample(t_diffuse, s_diffuse, tex_coord).xyz;

}