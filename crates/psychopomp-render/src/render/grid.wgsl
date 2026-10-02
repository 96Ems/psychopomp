struct Camera {
    viewport: vec4<f32>,
    orbit: vec4<f32>,
    depth: vec4<f32>,
    background: vec4<f32>,
    ink: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var labels: texture_2d<f32>;
@group(0) @binding(2) var label_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) atlas: vec4<f32>,
    @location(3) label: vec4<f32>,
    @location(4) size: vec4<f32>,
    @location(5) @interpolate(flat) face: u32,
    @location(6) local: vec2<f32>,
    @location(8) fill: vec4<f32>,
    @location(9) text_disclosure: vec4<f32>,
    @location(10) text_distance: vec2<f32>,
    @location(11) label_layout: vec4<f32>,
}

@vertex fn vertex_main(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
    @location(0) center: vec4<f32>,
    @location(1) size: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) atlas: vec4<f32>,
    @location(4) label: vec4<f32>,
    @location(5) reveal: vec4<f32>,
    @location(6) trim: vec4<f32>,
    @location(7) fill: vec4<f32>,
    @location(8) text_disclosure: vec4<f32>,
    @location(9) text_clip_a: vec4<f32>,
    @location(10) text_clip_b: vec4<f32>,
    @location(11) label_layout: vec4<f32>,
) -> VertexOut {
    let corners = array<vec2<f32>,6>(
        vec2(-0.5,-0.5),vec2(0.5,-0.5),vec2(0.5,0.5),
        vec2(-0.5,-0.5),vec2(0.5,0.5),vec2(-0.5,0.5));
    let q = corners[vertex % 6u];
    let face = vertex / 6u;
    var point = vec3(q, 0.5);
    switch face {
        case 1u: { point = vec3(-q.x,q.y,-0.5); }
        case 2u: { point = vec3(-0.5,q.y,q.x); }
        case 3u: { point = vec3(0.5,q.y,-q.x); }
        case 4u: { point = vec3(q.x,0.5,-q.y); }
        case 5u: { point = vec3(q.x,-0.5,q.y); }
        default: {}
    }
    // Clip growth from the fixed leading corner. Adjacent cells meet exactly;
    // there is no per-cell scale/pop or gap between the grid lines.
    point = vec3(
        mix(trim.x-0.5,trim.x+reveal.x-0.5,point.x+0.5),
        mix(0.5-trim.y-reveal.y,0.5-trim.y,point.y+0.5),
        mix(0.5-trim.z-reveal.z,0.5-trim.z,point.z+0.5));
    let local = point.xy * size.xy;
    point = center.xyz + point * size.xyz;
    let sy = sin(camera.orbit.x); let cy = cos(camera.orbit.x);
    let sp = sin(camera.orbit.y); let cp = cos(camera.orbit.y);
    let turned = vec3(cy*point.x + sy*point.z, point.y, -sy*point.x + cy*point.z);
    var view = vec3(turned.x, cp*turned.y - sp*turned.z, sp*turned.y + cp*turned.z);
    // Group headers are flat text, not opaque backing cards.
    if size.w == 1. { view = vec3(point.xy,-5000.+point.z); }
    if size.w == 2. {
        let anchor = vec3(cy*center.x+sy*center.z,center.y,-sy*center.x+cy*center.z);
        view = vec3(anchor.x+local.x,cp*anchor.y-sp*anchor.z+local.y,sp*anchor.y+cp*anchor.z);
    }
    let pixel = camera.viewport.xy * camera.viewport.zw + vec2(view.x,-view.y)*camera.orbit.z;
    var out: VertexOut;
    out.position = vec4(pixel.x / camera.viewport.x * 2.-1., 1.-pixel.y/camera.viewport.y*2., 0.5-(view.z-camera.depth.x)*camera.depth.y, 1.);
    // During regrouping, two retained front faces can occupy the same plane.
    // Stable catalog order resolves that tie without moving their geometry.
    out.position.z -= f32(instance) * 0.001 * camera.depth.y;
    if size.w > 0.5 { out.position.z = 0.; }
    out.face = face; out.size = size;
    out.color = color; out.atlas = atlas; out.label = label;
    out.local = local;
    out.fill = fill;
    out.text_disclosure = text_disclosure;
    out.label_layout = label_layout;
    out.text_distance = vec2(dot(vec3(local,1.),text_clip_a.xyz),dot(vec3(local,1.),text_clip_b.xyz));
    return out;
}

fn label_ink(in: VertexOut) -> f32 {
    let label_uv = vec2(in.local.x-in.label_layout.x, -(in.local.y-in.label.z-in.label_layout.y))/in.label.xy+vec2(0.5);
    // Sample unconditionally so derivatives stay uniform; the tile margin and
    // explicit inside test prevent bleeding from neighboring labels.
    let alpha = textureSample(labels,label_sampler,in.atlas.xy+clamp(label_uv,vec2(0.001),vec2(0.999))*in.atlas.zw).r;
    let inside = all(label_uv >= vec2(0.)) && all(label_uv <= vec2(1.)) && in.face == 0u
        && (in.label_layout.w < 0.5 || abs(in.local.x) <= in.label_layout.z);
    let ink_alpha = select(0.,alpha*in.label.w,inside);
    var ink = ink_alpha*in.color.a;
    // The grid edge is the aperture, not a second animation. Derivatives keep
    // the linear feather eight output pixels wide through rotation and zoom.
    // Only ink is masked; the opaque material and its border remain untouched.
    let dx = dpdx(in.text_distance); let dy = dpdy(in.text_distance);
    let pixel_width = sqrt(dx*dx+dy*dy);
    let feather_width = max(in.text_disclosure.y*pixel_width,vec2(0.00001));
    if in.text_disclosure.z > 0. {
        let coverage_xy = clamp(in.text_distance/feather_width,vec2(0.),vec2(1.));
        let mask = min(coverage_xy.x,coverage_xy.y);
        ink = select(0.,alpha*in.text_disclosure.x*mask,inside);
    }
    return ink;
}

@fragment fn fragment_main(in: VertexOut) -> @location(0) vec4<f32> {
    let ink = label_ink(in);
    // This pass owns opaque cell material and its attached ink only. Headings
    // must neither paint background-colored glyphs nor write into cell depth.
    if in.size.w > 0.5 { discard; }
    let lighting = array<f32,6>(1.,0.78,0.78,0.86,1.24,0.68);
    let background = camera.background.rgb;
    // Unfilled tables still own opaque depth. Their material matches the clear
    // color instead of becoming transparent or acquiring a lit checker pattern.
    let material = select(in.fill.rgb*lighting[in.face],in.fill.rgb,camera.depth.w >= 4.);
    let surface = mix(background,material,in.fill.a);
    let color = mix(surface,camera.ink.rgb,ink);
    return vec4(color,1.);
}

@fragment fn fragment_heading(in: VertexOut) -> @location(0) vec4<f32> {
    let ink = label_ink(in);
    if in.size.w <= 0.5 || ink <= 0. { discard; }
    // Premultiplied ink overlays the completed material and strokes. Zero
    // opacity is the existing frame, not the scene's dark background color.
    return vec4(camera.ink.rgb*ink,ink);
}
