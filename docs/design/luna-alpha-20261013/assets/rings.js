/* Три каменные кольцевые ленты с непрерывной фактурой и светящимися рунами.
   Их радиальные оболочки не пересекаются при любом вращении; центр пустой. */
window.LunaRings = class {
 constructor(canvas, frozen = false) {
  this.canvas = canvas; this.frozen = frozen; this.frame = 0; this.start = performance.now();
  this.reduced = matchMedia('(prefers-reduced-motion: reduce)');
  this.gl = canvas.getContext('webgl', {alpha: true, antialias: true, premultipliedAlpha: false});
  if (!this.gl) throw new Error('LunaRings: WebGL unavailable');
  this.pulses = Array.from({length: 3}, () => Array.from({length: 16}, () => { const duration = 5 + Math.random() * 5; return {start: -Math.random() * duration, duration}; }));
  this.setup();
  this.onVisibility = () => this.update();
  document.addEventListener('visibilitychange', this.onVisibility);
  this.reduced.addEventListener('change', this.onVisibility); this.update();
 }
 destroy() {
  this.disposed = true; this.textureImage.onload = null;
  cancelAnimationFrame(this.frame);
  document.removeEventListener('visibilitychange', this.onVisibility);
  this.reduced.removeEventListener('change', this.onVisibility);
  this.meshes.forEach(mesh => this.gl.deleteBuffer(mesh.buffer)); this.gl.deleteProgram(this.program); this.gl.deleteTexture(this.stoneTexture);
 }
 update() {
  cancelAnimationFrame(this.frame);
  this.draw(this.frozen || this.reduced.matches ? 2.4 : (performance.now() - this.start) / 1000);
  if (!document.hidden && !this.frozen && !this.reduced.matches) this.frame = requestAnimationFrame(() => this.update());
 }
 setup() {
  const gl = this.gl;
  const shader = (type, source) => {
   const s = gl.createShader(type); gl.shaderSource(s, source); gl.compileShader(s);
   if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s;
  };
  const vertex = shader(gl.VERTEX_SHADER, `
   attribute vec3 position; attribute vec3 normal; attribute vec3 color; attribute float emission; attribute float runeIndex;
   uniform float runeGlows[16];
   uniform mat3 rotation; uniform vec2 scale;
   varying float vGlow; varying vec3 vLocalNormal; varying vec3 vWorld; varying vec3 vNormal; varying vec3 vColor; varying vec3 vLocal; varying float vEmission;
   void main() {
    vec3 p = rotation * position;
    gl_Position = vec4(p.x * scale.x, -p.y * scale.y, -p.z * .35, 1.0);
    vGlow = 0.0;
    for (int i = 0; i < 16; i++) { if (abs(runeIndex-float(i)) < .5) vGlow = runeGlows[i]; }
    vLocalNormal = normal; vWorld = p; vNormal = rotation * normal; vColor = color; vLocal = position; vEmission = emission;
   }`);
  const fragment = shader(gl.FRAGMENT_SHADER, `
   #ifdef GL_FRAGMENT_PRECISION_HIGH
   precision highp float;
   #else
   precision mediump float;
   #endif
   uniform sampler2D stoneTexture; uniform float ownRing;
   uniform mat3 inverse0; uniform mat3 inverse1; uniform mat3 inverse2;
   varying float vGlow; varying vec3 vLocalNormal; varying vec3 vWorld; varying vec3 vNormal; varying vec3 vColor; varying vec3 vLocal; varying float vEmission;
   // Три проекции исходной фотографии смешиваются по нормали. Фактура
   // закреплена в координатах кольца, не растягивается вдоль тонких торцов.
   // Зеркальное повторение сохраняет края текстуры произвольного размера без жёсткого шва.
   vec2 stoneUV(vec2 uv) { return 1.0-abs(mod(uv,2.0)-1.0); }
   float stoneTone() {
    vec3 weights = pow(abs(normalize(vLocalNormal)),vec3(4.0));
    weights /= max(.001,weights.x+weights.y+weights.z);
    vec3 p = vLocal * .72 + vec3(.3,.5,.1);
    return dot(weights,vec3(texture2D(stoneTexture,stoneUV(p.yz)).r,
                            texture2D(stoneTexture,stoneUV(p.xz)).r,
                            texture2D(stoneTexture,stoneUV(p.xy)).r));
   }
   // Пересечение луча света с объёмом кольцевой ленты: цилиндр минус
   // внутреннее отверстие, ограниченный боковыми плоскостями. Три близких
   // направления смягчают тени соседних колец без отдельной карты глубины.
   float occludes(vec3 p, vec3 d, float radius) {
    float a = dot(d.xy,d.xy), b = dot(p.xy,d.xy);
    float outer = radius + .055, inner = radius - .055;
    if (a < .00001) return 0.0;
    float disc = b*b-a*(dot(p.xy,p.xy)-outer*outer);
    if (disc < 0.0) return 0.0;
    float lo = max(.015,(-b-sqrt(disc))/a), hi = (-b+sqrt(disc))/a;
    if (abs(d.z) < .00001) { if (abs(p.z) > .135) return 0.0; }
    else {
     float z0 = (-.135-p.z)/d.z, z1 = (.135-p.z)/d.z;
     lo = max(lo,min(z0,z1)); hi = min(hi,max(z0,z1));
    }
    if (hi <= lo) return 0.0;
    float hole = b*b-a*(dot(p.xy,p.xy)-inner*inner);
    if (hole < 0.0) return 1.0;
    float h0 = (-b-sqrt(hole))/a, h1 = (-b+sqrt(hole))/a;
    return (lo < h0 || hi > h1) ? 1.0 : 0.0;
   }
   float shadowRay(vec3 p, vec3 d) {
    float hit = 0.0;
    if (ownRing > .5) hit = max(hit,occludes(inverse0*p,inverse0*d,1.0));
    if (abs(ownRing-1.0) > .5) hit = max(hit,occludes(inverse1*p,inverse1*d,1.2672599504));
    if (ownRing < 1.5) hit = max(hit,occludes(inverse2*p,inverse2*d,1.5339610719));
    return hit;
   }
   void main() {
    vec3 n = normalize(vNormal);
    float pulse = vGlow;
    if (vEmission > .5) {
     // При уходе грани за ребро свет исчезает постепенно, без отсечки кадра.
     float facing = smoothstep(0.0, .20, n.z);
     if (facing <= .001) discard;
     if (vEmission > 2.5) gl_FragColor = vec4(vColor, .12 * pulse * facing);
     else if (vEmission > 1.5) gl_FragColor = vec4(vColor, .30 * pulse * facing);
     else gl_FragColor = vec4(mix(vec3(.57,.60,.63),vColor,pulse), mix(.015,.98,pulse) * facing);
     return;
    }
    float key = max(0.0, dot(n, normalize(vec3(-.4, -.65, 1.0))));
    float fill = max(0.0, dot(n, normalize(vec3(.8, .3, .2))));
    vec3 ray = normalize(vec3(-.4,-.65,1.0));
    vec3 p = vWorld + n * .006;
    float shadow = (shadowRay(p,ray) + shadowRay(p,normalize(ray+vec3(.045,.025,0)))
                   + shadowRay(p,normalize(ray-vec3(.045,.025,0)))) / 3.0;
    float light = .48 + .48 * key * (1.0-.70*shadow) + .15 * fill;
    float texture = mix(.62,1.34,stoneTone());
    vec3 tint = vec3(1.0, .998, .985);
    gl_FragColor = vec4(vColor * tint * light * texture, 1.0);
   }`);
  this.program = gl.createProgram(); gl.attachShader(this.program, vertex); gl.attachShader(this.program, fragment);
  gl.linkProgram(this.program); gl.deleteShader(vertex); gl.deleteShader(fragment);
  if (!gl.getProgramParameter(this.program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(this.program));
  this.attributes = ['position', 'normal', 'color', 'emission', 'runeIndex'].map(name => gl.getAttribLocation(this.program, name));
  this.rotation = gl.getUniformLocation(this.program, 'rotation'); this.scale = gl.getUniformLocation(this.program, 'scale');
  this.glows = gl.getUniformLocation(this.program, 'runeGlows[0]');
  this.ownRing = gl.getUniformLocation(this.program, 'ownRing');
  this.inverses = [0,1,2].map(k => gl.getUniformLocation(this.program, 'inverse'+k));
  gl.enable(gl.DEPTH_TEST); gl.depthFunc(gl.LEQUAL); gl.clearColor(0, 0, 0, 0);
  gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
  this.meshes = [0, 1, 2].map(k => this.makeMesh(k));
  this.stoneTexture = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D,this.stoneTexture);
  // До загрузки PNG используется нейтральный серый пиксель. Размер исходника
  // не обязан быть степенью двойки: WebGL 1 допускает LINEAR и CLAMP_TO_EDGE.
  gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,1,1,0,gl.RGBA,gl.UNSIGNED_BYTE,new Uint8Array([160,160,160,255]));
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
  this.textureImage = new Image();
  this.textureImage.onload = () => {
   if (this.disposed) return;
   gl.bindTexture(gl.TEXTURE_2D,this.stoneTexture);
   gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,this.textureImage);
   this.update();
  };
  this.textureImage.src = 'assets/stone-reference.png';
 }
 makeMesh(k) {
  const gl = this.gl, data = [], N = 192, radius = [1.00, 1.2672599504, 1.5339610719][k];
  const outer = radius + .055, inner = radius - .055, halfWidth = .135, bevel = .014;
  const section = [[outer,-halfWidth+bevel],[outer,halfWidth-bevel], [outer-bevel,halfWidth],
   [inner+bevel,halfWidth],[inner,halfWidth-bevel],[inner,-halfWidth+bevel],
   [inner+bevel,-halfWidth],[outer-bevel,-halfWidth]];
  const triangle = (points, normal, color, emission = 0, runeIndex = -1) => {
   points.forEach((p, i) => data.push(...p, ...(Array.isArray(normal[0]) ? normal[i] : normal), ...color, emission, runeIndex));
  };
  const local = (a, r, z) => [Math.cos(a)*r, Math.sin(a)*r, z];
  for (let i = 0; i < N; i++) {
   const a = i*2*Math.PI/N, b = (i+1)*2*Math.PI/N;
   for (let j = 0; j < section.length; j++) {
    const s = section[j], e = section[(j+1)%section.length], dr = e[0]-s[0], dz = e[1]-s[1], len = Math.hypot(dr,dz);
    const normalAt = angle => [Math.cos(angle)*dz/len, Math.sin(angle)*dz/len, -dr/len];
    const na = normalAt(a), nb = normalAt(b);
    const points = [local(a,...s),local(b,...s),local(b,...e),local(a,...e)];
    // Общий серый материал и плавные нормали скрывают прямоугольные сектора.
    // Светлая каменная фаска остаётся отдельной геометрической поверхностью.
    const base = j===0 ? [.62,.65,.68] : j===4 ? [.51,.54,.57] : j===2||j===6 ? [.80,.82,.83] : [.73,.76,.78];
    triangle([points[0],points[1],points[2]], [na,nb,nb], base);
    triangle([points[0],points[2],points[3]], [na,nb,na], base);
   }
  }
  const stoneCount = data.length / 11;
  // Восемь нарисованных вручную знаков. Координаты принадлежат широкой
  // наружной грани; это геометрия гравировки, а не экранный шрифт.
  const glyphs = [
   [[[-.7,-.5],[0,-1],[.7,-.5],[0,0],[-.7,-.5]],[[0,-1],[0,1]],[[0,.25],[.65,.7],[0,1]]],
   [[[0,-1],[0,1]],[[-.65,-.65],[.65,-.1],[-.65,.4]],[[0,.4],[.6,.85]]],
   [[[0,-1],[-.75,0],[0,.8],[.75,0],[0,-1]],[[0,.8],[0,1.1]]],
   [[[-.6,-1],[-.6,1]],[[-.6,-.7],[.7,-.15],[-.6,.35]],[[-.6,.35],[.65,.85]]],
   [[[-.7,-.85],[.7,.85]],[[-.7,.85],[.7,-.85]],[[0,-1.05],[0,1.05]]],
   [[[-.6,-.8],[.5,-.8],[.65,0],[0,.55],[-.65,0],[-.6,-.8]],[[0,.55],[0,1]]],
   [[[-.65,-1],[-.65,.2],[0,.85],[.65,.2],[.65,-1]],[[0,-.5],[0,1.1]]],
   [[[0,-1],[-.7,-.15],[.7,.25],[0,1]], [[-.65,.9],[.65,-.9]]]
  ];
  for (let i = 0; i < 16; i++) {
   const facet = i*12 + 3, a = (facet+.5)*2*Math.PI/N;
   const radial = [Math.cos(a),Math.sin(a),0];
   // Знаки огибают наружную поверхность, а не лежат в касательной плоскости.
   // Малый отступ не даёт им пересекать камень при движении камеры/кольца.
   const point = (x,y) => local(a + x*.054/outer, outer + .005, y*.096);
   const paths = glyphs[(i*3+k*2)%glyphs.length];
   paths.forEach(path => {
    for(let j=0;j<path.length-1;j++) {
     const p=path[j], q=path[j+1], dx=(q[0]-p[0])*.054, dy=(q[1]-p[1])*.096;
     const len=Math.hypot(dx,dy);
     // Широкий полупрозрачный ореол и тонкая светлая сердцевина принадлежат
     // той же наружной грани. Буфер глубины не даёт свету проходить сквозь камень.
     for (const [width, emission, color] of [[.044,3,[.20,.85,.80]],[.025,2,[.25,.90,.85]],[.012,1,[.65,1.0,.94]]]) {
      const ox=-dy/len*width/.054, oy=dx/len*width/.096;
      const pts=[point(p[0]+ox,p[1]+oy),point(q[0]+ox,q[1]+oy),point(q[0]-ox,q[1]-oy),point(p[0]-ox,p[1]-oy)];
      triangle([pts[0],pts[1],pts[2]],radial,color,emission,i);
      triangle([pts[0],pts[2],pts[3]],radial,color,emission,i);
     }
    }
   });
  }
  const buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buffer); gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW);
  return {buffer, count: data.length/11, stoneCount};
 }
 matrix(k, t) {
  const normalize = v => {const l=Math.hypot(...v); return v.map(x=>x/l);};
  const cross = (a,b) => [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
  const normal=normalize([[0,.93,.367],[.86,-.2,.47],[-.72,-.27,.639]][k]);
  const u=normalize(cross([0,1,0],normal)), v=cross(normal,u);
  const axis=normalize([[.27,.68,1],[.9,-.28,.72],[-.58,.85,.63]][k]);
  const angle=(t-2.4)*2*Math.PI/[12,17,23][k], cs=Math.cos(angle), sn=Math.sin(angle);
  // Формула Родригеса даёт полный оборот вокруг неподвижной оси. Базис
  // при 2.4 с задаёт читаемые три плоскости, а не имитацию покачивания.
  const turn=p=>{const dot=p.reduce((s,x,i)=>s+x*axis[i],0), c=cross(axis,p); return p.map((x,i)=>x*cs+c[i]*sn+axis[i]*dot*(1-cs));};
  return new Float32Array([...turn(u),...turn(v),...turn(normal)]);
 }
 pulse(k,i,t) {
  // Косинус начинается и заканчивается тусклым тоном с нулевой скоростью:
  // смена случайного периода не вызывает скачка яркости на стыке циклов.
  if (this.frozen || this.reduced.matches) return .5-.5*Math.cos(((i*7+k*3)%16)/16*2*Math.PI);
  const cycle = this.pulses[k][i];
  while (t >= cycle.start + cycle.duration) {
   cycle.start += cycle.duration; cycle.duration = 5 + Math.random() * 5;
  }
  return .5 - .5 * Math.cos((t-cycle.start) / cycle.duration * 2*Math.PI);
 }
 draw(t) {
  const c=this.canvas, gl=this.gl, dpr=Math.min(devicePixelRatio||1,2), w=c.clientWidth, h=c.clientHeight;
  if(!w||!h) return;
  if(c.width!==Math.round(w*dpr)||c.height!==Math.round(h*dpr)){c.width=Math.round(w*dpr);c.height=Math.round(h*dpr);}
  gl.viewport(0,0,c.width,c.height); gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT); gl.useProgram(this.program); gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D,this.stoneTexture);
  const scale=Math.min(w,h)*.276; gl.uniform2f(this.scale,scale*2/w,scale*2/h);
  const matrices = [0,1,2].map(k => this.matrix(k,t));
  matrices.forEach((m,k) => gl.uniformMatrix3fv(this.inverses[k],false,new Float32Array([m[0],m[3],m[6],m[1],m[4],m[7],m[2],m[5],m[8]])));
  const renderMesh = (mesh,k,start,count) => {
   gl.bindBuffer(gl.ARRAY_BUFFER,mesh.buffer);
   let offset=0;
   this.attributes.forEach((a,i)=>{const size=i>=3?1:3; gl.enableVertexAttribArray(a); gl.vertexAttribPointer(a,size,gl.FLOAT,false,44,offset*4);offset+=size;});
   gl.uniformMatrix3fv(this.rotation,false,matrices[k]); gl.uniform1f(this.ownRing,k); gl.uniform1fv(this.glows,new Float32Array(Array.from({length:16},(_,i)=>this.pulse(k,i,t)))); gl.drawArrays(gl.TRIANGLES,start,count);
  };
  // Сначала весь камень заполняет буфер глубины. Прозрачные ореолы и знаки
  // проверяют эту глубину, но не записывают свою: пересечение штрихов не мерцает.
  gl.depthMask(true);
  this.meshes.forEach((mesh,k)=>renderMesh(mesh,k,0,mesh.stoneCount));
  gl.depthMask(false);
  this.meshes.forEach((mesh,k)=>renderMesh(mesh,k,mesh.stoneCount,mesh.count-mesh.stoneCount));
  gl.depthMask(true);
 }
};
