struct Node { pose:vec4<f32>, optics:vec4<f32>, content:vec4<f32>, label_a:vec4<f32>, label_b:vec4<f32>, ink:vec4<f32>, border:vec4<f32>, volume:vec4<f32> }
struct Link { ends:vec4<f32>, state:vec4<f32>, depth:vec4<f32> }
struct Frame { viewport:vec4<f32>, background:vec4<f32>, surface:vec4<f32>, accent:vec4<f32>, wire:vec4<f32>, nodes:array<Node,16>, links:array<Link,32> }
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var atlas:texture_2d<f32>;
@group(0) @binding(2) var ink_sampler:sampler;
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) @interpolate(linear,sample) point:vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i:u32)->Vertex {
    let p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.))[i];
    var v:Vertex;v.position=vec4(p,0.,1.);v.point=(vec2(p.x,-p.y)+1.)*.5*frame.viewport.xy;return v;
}
fn over(a:vec4<f32>,b:vec4<f32>)->vec4<f32> {let alpha=b.a+a.a*(1.-b.a);if alpha<=0. {return vec4(0.);}return vec4((b.rgb*b.a+a.rgb*a.a*(1.-b.a))/alpha,alpha);}
fn segment(p:vec2<f32>,a:vec2<f32>,b:vec2<f32>)->f32 {let d=b-a;let t=clamp(dot(p-a,d)/max(dot(d,d),0.000001),0.,1.);return length(p-a-d*t);}
fn projected(p:vec2<f32>)->vec2<f32> {if frame.viewport.z<.5 {return p;}return vec2((p.x-p.y)*0.8660254038,(p.x+p.y)*.5);}
fn inverse(p:vec2<f32>)->vec2<f32> {if frame.viewport.z<.5 {return p;}return vec2(p.x*0.5773502692+p.y,-p.x*0.5773502692+p.y);}
fn glyph(rect:vec4<f32>,p:vec2<f32>)->f32 {
    let q=p+rect.zw*.5;
    if rect.z<=0. || any(q<vec2(-1.)) || any(q>rect.zw+vec2(2.)) {return 0.;}
    return textureSampleLevel(atlas,ink_sampler,(rect.xy+q)/vec2<f32>(textureDimensions(atlas)),0.).r;
}
fn soft_glyph(rect:vec4<f32>,p:vec2<f32>,blur:f32)->f32 {
    if blur<.01 {return glyph(rect,p);}
    if any(abs(p)>rect.zw*.5+blur+2.) {return 0.;}
    let offsets=array<f32,3>(-blur,0.,blur);let weights=array<f32,3>(1.,2.,1.);var coverage=0.;
    for(var y=0u;y<3u;y++){for(var x=0u;x<3u;x++){coverage+=glyph(rect,p+vec2(offsets[x],offsets[y]))*weights[x]*weights[y]/16.;}}
    return coverage;
}
fn node_ink(n:Node,p:vec2<f32>)->vec4<f32> {
    let half=n.pose.zw*.5;let local=inverse(p);let depth=n.optics.w;
    var hit=false;var shade=1.;
    if all(abs(local)<=half) {hit=true;}
    else if depth>0. {
        let delta=max(local.x-half.x,local.y-half.y);
        let side=local-vec2(delta);
        if delta>=0. && delta<=depth && all(side>=-half) && all(side<=half+vec2(.0001)) {hit=true;shade=select(.64,.8,local.x-half.x>local.y-half.y);}
    }
    let a=projected(vec2(-half.x,-half.y));let b=projected(vec2(half.x,-half.y));
    let c=projected(half);let d=projected(vec2(-half.x,half.y));let down=vec2(0.,depth);
    let top_edge=min(min(segment(p,a,b),segment(p,b,c)),min(segment(p,c,d),segment(p,d,a)));
    var edge=top_edge;
    if depth>0. {edge=min(edge,min(min(segment(p,b,b+down),segment(p,c,c+down)),segment(p,d,d+down)));edge=min(edge,min(segment(p,b+down,c+down),segment(p,c+down,d+down)));}
    let stroke=clamp(1.5-edge*n.optics.x,0.,1.);
    var face=frame.surface.rgb*shade;
    var border=n.border.rgb;
    if frame.viewport.z>.5 {
        let material=mix(frame.surface.rgb,frame.accent.rgb*.16,n.volume.y*.5);
        let light=clamp(.5-.18*local.x/max(half.x,1.)-.16*local.y/max(half.y,1.),0.,1.);
        face=material*shade+vec3(.012,.015,.023)*light*shade;
        border=mix(n.border.rgb*.78,mix(n.border.rgb,vec3(.63,.67,.76),.18),select(0.,1.,top_edge<=edge+.0001));
    }
    var result=vec4(face,select(0.,n.content.x,hit));
    result=over(result,vec4(border,stroke*n.content.x));
    let primary=select(1.,clamp(1.-n.content.z*2.,0.,1.),n.label_b.z>0.);
    let secondary=clamp(n.content.z*2.-1.,0.,1.);
    let alpha_a=primary*n.content.y;let alpha_b=secondary*n.content.y;
    // Billboard labels stay readable; their center follows the projected top face.
    // Disclose ink inside the growing top face instead of scaling the type or
    // letting a full-width word float outside a narrow solid. One output-pixel
    // inward feather uses the edge normal's length, preserving fractional motion.
    var mask=1.;
    if frame.viewport.z>.5 {let room=half-abs(local);mask=clamp(min(room.x,room.y)*.8660254038*n.optics.x,0.,1.);}
    result=over(result,vec4(n.ink.rgb,soft_glyph(n.label_a,p,(1.-alpha_a)*4.)*alpha_a*mask));
    if alpha_b>0. {result=over(result,vec4(frame.accent.rgb,soft_glyph(n.label_b,p,(1.-alpha_b)*4.)*alpha_b*mask));}
    return result;
}
fn node_sample(n:Node,point:vec2<f32>)->vec4<f32> {
    let p=(point-n.pose.xy)/n.optics.x;let blur=n.optics.y;
    if blur<=.2 {return node_ink(n,p);}
    // Dense support prevents the wide entrance blur reading as three displaced
    // copies of each word/edge. The sampled blur pose and its timing do not change.
    let offsets=array<f32,5>(-blur*.72,-blur*.36,0.,blur*.36,blur*.72);let weights=array<f32,5>(1.,4.,6.,4.,1.);var premultiplied=vec4(0.);
    for(var y=0u;y<5u;y++){for(var x=0u;x<5u;x++){let c=node_ink(n,p+vec2(offsets[x],offsets[y]));let weight=weights[x]*weights[y]/256.;premultiplied+=vec4(c.rgb*c.a,c.a)*weight;}}
    if premultiplied.a<=0. {return vec4(0.);}return vec4(premultiplied.rgb/premultiplied.a,premultiplied.a);
}
fn linear(c:vec3<f32>)->vec3<f32> {return select(c/12.92,pow((c+.055)/1.055,vec3(2.4)),c>vec3(.04045));}
fn wire_visibility(l:Link,p:vec2<f32>)->f32 {
    // At a fixed isometric screen point, larger world Z is nearer the camera.
    // Compare the sampled wire height with the ray's actual top/side-face hit,
    // not the object's center or a blanket "all wires behind all boxes" rule.
    let d=l.ends.zw-l.ends.xy;
    let t=clamp(dot(p-l.ends.xy,d)/max(dot(d,d),.000001),0.,1.);
    let z=mix(l.depth.x,l.depth.y,t)+.001;
    var visible=1.;
    for(var j=0u;j<u32(frame.viewport.w);j++) {
        let n=frame.nodes[j];if n.optics.z<=0. || n.content.x<=0. {continue;}
        let local=inverse((p-n.pose.xy)/n.optics.x);
        let half=n.pose.zw*.5;
        let drop=max(max(local.x-half.x,local.y-half.y),0.);
        let face_z=n.volume.x-clamp(drop,0.,n.optics.w)*n.optics.x;
        if face_z<=z {continue;}
        let side=local-vec2(drop);
        let support=(n.optics.y+2.);
        if drop>n.optics.w+support || any(side < -half-support) || any(side>half+support) {continue;}
        visible*=1.-node_sample(n,p).a*n.optics.z;
    }
    return visible;
}
fn wires(initial:vec4<f32>,p:vec2<f32>,depth_aware:bool)->vec4<f32> {
    var color=initial;
    for(var i=0u;i<u32(frame.wire.w);i++) {
        let l=frame.links[i];let draw=l.state.x;if draw<=0. || l.state.y<=0. {continue;}
        let a=l.ends.xy;let b=mix(a,l.ends.zw,draw);let ink=mix(frame.wire.rgb,frame.accent.rgb,l.state.z);
        let distance=segment(p,a,b);if distance>2.1 {continue;}
        var visibility=1.;if depth_aware {visibility=wire_visibility(l,p);}
        color=over(color,vec4(ink,clamp(1.5-distance,0.,1.)*l.state.y*visibility));
        let comet=4.*draw*(1.-draw)*l.state.y*visibility;
        for(var j=0u;j<3u;j++) {let tail=array<f32,3>(.18,.09,.035)[j];let width=array<f32,3>(1.,1.2,1.6)[j];let strength=array<f32,3>(.22,.5,1.)[j];let start=mix(a,l.ends.zw,max(0.,draw-tail));color=over(color,vec4(mix(frame.accent.rgb,vec3(.95),.65),clamp(width+.5-segment(p,start,b),0.,1.)*comet*strength));}
    }
    return color;
}
@fragment fn fs(v:Vertex)->@location(0) vec4<f32> {
    var color=frame.background;
    if frame.viewport.z<.5 {color=wires(color,v.point,false);}
    for(var i=0u;i<u32(frame.viewport.w);i++) {
        let n=frame.nodes[i];if n.optics.z<=0. {continue;}
        let half=n.pose.zw*n.optics.x*.5;var extent=half;
        if frame.viewport.z>.5 {extent=vec2((half.x+half.y)*.8660254038,(half.x+half.y)*.5+n.optics.w*n.optics.x);}
        let delta=v.point-n.pose.xy;let pad=(n.optics.y+32.)*n.optics.x;
        if any(abs(delta)>extent+pad) {continue;}
        if n.content.w>0. {
            let q=abs(inverse(delta)/n.optics.x)-n.pose.zw*.5;
            let distance=length(max(q,vec2(0.)))+min(max(q.x,q.y),0.);
            color=over(color,vec4(frame.accent.rgb,exp(-.5*pow(distance/10.,2.))*.09*n.content.w*n.optics.z));
        }
        var node=node_sample(n,v.point);node=vec4(min(node.rgb*(1.+.7*clamp(n.optics.y/12.,0.,1.)),vec3(1.)),node.a*n.optics.z);
        color=over(color,node);
    }
    if frame.viewport.z>.5 {color=wires(color,v.point,true);}
    return vec4(linear(color.rgb),1.);
}
