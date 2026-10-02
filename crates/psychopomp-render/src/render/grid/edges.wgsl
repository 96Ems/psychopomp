struct Camera {
    viewport: vec4<f32>,
    orbit: vec4<f32>,
    depth: vec4<f32>,
    background: vec4<f32>,
    ink: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

fn project(p: vec3<f32>) -> vec3<f32> {
    let sy=sin(camera.orbit.x); let cy=cos(camera.orbit.x);
    let sp=sin(camera.orbit.y); let cp=cos(camera.orbit.y);
    let t=vec3(cy*p.x+sy*p.z,p.y,-sy*p.x+cy*p.z);
    return vec3(t.x,cp*t.y-sp*t.z,sp*t.y+cp*t.z);
}

struct Edge {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(linear, sample) local: vec2<f32>,
    @location(1) @interpolate(flat) length: f32,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) lower: vec3<f32>,
    @location(4) @interpolate(flat) upper: vec3<f32>,
    @location(5) @interpolate(flat) depths: vec2<f32>,
    @location(7) @interpolate(flat) tangent: vec2<f32>,
}

@vertex fn vertex_main(
    @builtin(vertex_index) vertex: u32,
    @location(0) center: vec4<f32>,
    @location(1) size: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(5) reveal: vec4<f32>,
    @location(6) trim: vec4<f32>,
) -> Edge {
    let corners=array<vec2<f32>,6>(vec2(0.,-1.),vec2(1.,-1.),vec2(1.,1.),vec2(0.,-1.),vec2(1.,1.),vec2(0.,1.));
    let q=corners[vertex%6u];
    let edge=vertex/6u;
    let axis=edge/4u; let side=edge%4u;
    let a1=(axis+1u)%3u; let a2=(axis+2u)%3u;
    let lower=center.xyz+vec3(trim.x-0.5,0.5-trim.y-reveal.y,0.5-trim.z-reveal.z)*size.xyz;
    let upper=center.xyz+vec3(trim.x+reveal.x-0.5,0.5-trim.y,0.5-trim.z)*size.xyz;
    var a=lower; var b=lower;
    a[a1]=select(lower[a1],upper[a1],(side&1u)!=0u);
    a[a2]=select(lower[a2],upper[a2],(side&2u)!=0u);
    b=a; b[axis]=upper[axis];
    var n1=vec3(0.); var n2=vec3(0.);
    n1[a1]=select(-1.,1.,(side&1u)!=0u);
    n2[a2]=select(-1.,1.,(side&2u)!=0u);
    let rules=u32(camera.depth.w)&3u;
    // Row rules belong to the front table plane, not the back rim of its slab.
    let visible=max(project(n1).z,project(n2).z)>0. && (rules==0u || (rules==1u && axis==0u && (side&2u)!=0u));
    let va=project(a); let vb=project(b);
    let pa=vec2(va.x,-va.y)*camera.orbit.z;
    let pb=vec2(vb.x,-vb.y)*camera.orbit.z;
    let len=length(pb-pa);
    let tangent=(pb-pa)/max(len,0.00001);
    let normal=vec2(-tangent.y,tangent.x);
    // Rasterize beyond the analytic feather so MSAA does not attenuate it twice.
    let radius=camera.orbit.w*0.5+1.5;
    let local=vec2(mix(-radius,len+radius,q.x),q.y*radius);
    let pixel=camera.viewport.xy*camera.viewport.zw+pa+tangent*local.x+normal*local.y;
    var out: Edge;
    out.position=vec4(pixel.x/camera.viewport.x*2.-1.,1.-pixel.y/camera.viewport.y*2.,0.,1.);
    out.local=local; out.length=len;
    out.color=select(vec4(0.),vec4(color.rgb,color.a*reveal.w),visible && size.w<0.5 && len>0.00001);
    out.lower=lower-a; out.upper=upper-a; out.depths=vec2(va.z,vb.z);
    out.tangent=tangent;
    return out;
}

fn coverage(in: Edge) -> f32 {
    let nearest=vec2(clamp(in.local.x,0.,in.length),0.);
    return clamp(camera.orbit.w*0.5+0.5-length(in.local-nearest),0.,1.);
}

fn edge_depth(in: Edge) -> f32 {
    let sy=sin(camera.orbit.x); let cy=cos(camera.orbit.x);
    let sp=sin(camera.orbit.y); let cp=cos(camera.orbit.y);
    // Stay relative to the edge's first endpoint. Subtracting large canvas
    // coordinates loses precision, amplified by a nearly edge-on box face.
    // These varyings also locate the actual MSAA sample, not the pixel center.
    let delta=in.tangent*in.local.x+vec2(-in.tangent.y,in.tangent.x)*in.local.y;
    let xy=delta/camera.orbit.z*vec2(1.,-1.);
    // A centered stroke overlaps its own adjacent faces. Use the ray/box surface
    // depth there, not a large bias that would let rear edges show through.
    let origin=vec3(cy*xy.x+sy*sp*xy.y,cp*xy.y,sy*xy.x-cy*sp*xy.y);
    let direction=vec3(-sy*cp,sp,cy*cp);
    let inverse=select(vec3(-1.),vec3(1.),direction>=vec3(0.))/max(abs(direction),vec3(0.0000001));
    let t0=(in.lower-origin)*inverse; let t1=(in.upper-origin)*inverse;
    let low=min(t0,t1); let high=max(t0,t1);
    let entry=max(low.x,max(low.y,low.z)); let exit=min(high.x,min(high.y,high.z));
    var z=mix(in.depths.x,in.depths.y,clamp(in.local.x/max(in.length,0.00001),0.,1.));
    if entry<=exit { z=max(z,in.depths.x+exit); }
    // Clear the material pass's catalog-priority budget uniformly. Strokes
    // themselves must NOT inherit per-cell priority: coincident shared edges
    // union their coverage even when neighboring cells have different presence.
    return clamp(0.5-(z-camera.depth.x+0.005+camera.depth.z)*camera.depth.y,0.,1.);
}

@fragment fn fragment_depth(in: Edge) -> @builtin(frag_depth) f32 {
    if coverage(in)*in.color.a<=0.00001 { discard; }
    return edge_depth(in);
}

struct Ink {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
}
@fragment fn fragment_ink(in: Edge) -> Ink {
    let amount=coverage(in)*in.color.a;
    if amount<=0.00001 { discard; }
    let z=mix(in.depths.x,in.depths.y,clamp(in.local.x/max(in.length,0.00001),0.,1.));
    let alpha=amount*clamp(0.7+z*camera.orbit.z/1600.,0.32,0.95);
    var out: Ink;
    // Coincident edges can reconstruct a shared surface a few ULPs apart.
    // Union their coverage rather than flickering between the winning samples.
    out.depth=max(0.,edge_depth(in)-0.001*camera.depth.y);
    out.color=vec4(in.color.rgb*alpha,alpha);
    return out;
}
