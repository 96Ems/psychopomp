@group(0) @binding(0) var material: texture_2d<f32>;
@group(0) @binding(1) var strokes: texture_2d<f32>;

@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(p[index],0.,1.);
}

@fragment fn fragment_main(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p=vec2<i32>(pixel.xy);
    let base=textureLoad(material,p,0);
    let ink=textureLoad(strokes,p,0);
    return vec4(ink.rgb+base.rgb*(1.-ink.a),1.);
}
