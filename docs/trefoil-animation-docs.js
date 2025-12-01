// Trefoil animation for Valknut hero banner
document.addEventListener('DOMContentLoaded', function() {
  const canvas = document.getElementById('neural-network');
  if (!canvas) return;

  // Renderer - size to hero container, not viewport
  const renderer = new THREE.WebGLRenderer({canvas: canvas, antialias: true, alpha: true});
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
  
  const container = document.querySelector('.hero-container');
  function resizeRenderer() {
    if (container) {
      const { width, height } = container.getBoundingClientRect();
      renderer.setSize(width, height);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
    }
  }
  
  // Scene & camera
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 100);
  camera.position.set(0, 0, 3.15); // Zoomed out slightly (was 2.75)

  // Trefoil torus knot curve T(2,3)
  class TrefoilTorusCurve extends THREE.Curve {
    constructor(R=1.7, r=0.52){ super(); this.R = R; this.r = r; }
    getPoint(t, target=new THREE.Vector3()){
      const phi = t * Math.PI * 2.0;
      const rad = this.R + this.r * Math.cos(3*phi);
      target.set(rad*Math.cos(2*phi), rad*Math.sin(2*phi), this.r*Math.sin(3*phi));
      return target.multiplyScalar(0.40);
    }
  }
  const curve = new TrefoilTorusCurve();

  // Build the tube grid just to get a regular vertex lattice on the surface
  const tubularSegments = 360;   // segments along the knot
  const radialSegments  = 18;    // segments around circumference
  const tubeRadius      = 0.16;
  const tubeGeo = new THREE.TubeGeometry(curve, tubularSegments, tubeRadius, radialSegments, true);
  tubeGeo.rotateY(Math.PI/(radialSegments*2)); // move seam out of view

  // Extract grid vertex positions
  const posAttr = tubeGeo.getAttribute('position');
  const ringSize = radialSegments + 1;
  const rings = tubularSegments + 1;
  const totalVerts = rings * ringSize;

  // --- Build edge line segments with per-vertex colors ---
  // We'll add segments along u (rings) and along v (around circumference)
  const segmentsU = tubularSegments * ringSize;
  const segmentsV = tubularSegments * radialSegments;
  const totalSegs = segmentsU + segmentsV;

  const linePositions = new Float32Array(totalSegs * 2 * 3);
  const lineColors    = new Float32Array(totalSegs * 2 * 3);
  const lineUs        = new Float32Array(totalSegs * 2); // param u per vertex for animation
  const lineVs        = new Float32Array(totalSegs * 2); // param v around circumference
  const lineSeed      = new Float32Array(totalSegs * 2); // stable per-vertex hash

  function fract(x){ return x - Math.floor(x); }

  let ptr = 0;
  function copyVertex(index, uNorm){
    linePositions[ptr*3+0] = posAttr.getX(index);
    linePositions[ptr*3+1] = posAttr.getY(index);
    linePositions[ptr*3+2] = posAttr.getZ(index);
    // initial color mid-gray
    lineColors[ptr*3+0] = 0.5;
    lineColors[ptr*3+1] = 0.5;
    lineColors[ptr*3+2] = 0.5;
    lineUs[ptr] = uNorm;
    const vNorm = (index % ringSize) / radialSegments; // 0..1 around circumference
    lineVs[ptr] = vNorm;
    lineSeed[ptr] = fract(Math.sin(index * 12.9898) * 43758.5453); // stable per-vertex hash
    ptr++;
  }

  // Segments along u (connect ring i to i+1, same radial j)
  for(let i=0;i<tubularSegments;i++){
    const u0 = i / tubularSegments;
    const u1 = (i+1) / tubularSegments;
    for(let j=0;j<ringSize;j++){
      const idx0 = i*ringSize + j;
      const idx1 = (i+1)*ringSize + j;
      copyVertex(idx0, u0);
      copyVertex(idx1, u1);
    }
  }
  // Segments along v (connect j to j+1 within each ring i)
  for(let i=0;i<tubularSegments;i++){
    const u = i / tubularSegments;
    for(let j=0;j<radialSegments;j++){
      const idx0 = i*ringSize + j;
      const idx1 = i*ringSize + (j+1);
      copyVertex(idx0, u);
      copyVertex(idx1, u);
    }
  }

  const lineGeo = new THREE.BufferGeometry();
  lineGeo.setAttribute('position', new THREE.BufferAttribute(linePositions, 3));
  lineGeo.setAttribute('color',    new THREE.BufferAttribute(lineColors, 3));

  const lineMat = new THREE.LineBasicMaterial({
    vertexColors: true,
    transparent: true,
    opacity: 0.3, // Increased to make shimmer more visible
    blending: THREE.AdditiveBlending
  });
  const edges = new THREE.LineSegments(lineGeo, lineMat);
  scene.add(edges);

  // --- Build vertex points for visual pips at lattice vertices ---
  const pointsPositions = new Float32Array(totalVerts * 3);
  const pointsColors    = new Float32Array(totalVerts * 3);
  const pointsUs        = new Float32Array(totalVerts);
  const pointsVs        = new Float32Array(totalVerts);
  const pointsSeed      = new Float32Array(totalVerts);

  let pptr = 0;
  for(let i=0;i<rings;i++){
    const u = i / tubularSegments; // note tubularSegments for normalization; last ring == 1
    for(let j=0;j<ringSize;j++){
      const idx = i*ringSize + j;
      pointsPositions[pptr*3+0] = posAttr.getX(idx);
      pointsPositions[pptr*3+1] = posAttr.getY(idx);
      pointsPositions[pptr*3+2] = posAttr.getZ(idx);
      pointsColors[pptr*3+0] = 0.5;
      pointsColors[pptr*3+1] = 0.5;
      pointsColors[pptr*3+2] = 0.5;
      pointsUs[pptr] = Math.min(1.0, u); // clamp final duplicate
      pointsVs[pptr] = j / radialSegments;
      pointsSeed[pptr] = fract(Math.sin(idx * 12.9898) * 43758.5453);
      pptr++;
    }
  }

  const ptsGeo = new THREE.BufferGeometry();
  ptsGeo.setAttribute('position', new THREE.BufferAttribute(pointsPositions, 3));
  ptsGeo.setAttribute('color',    new THREE.BufferAttribute(pointsColors, 3));
  const ptsMat = new THREE.PointsMaterial({
    vertexColors: true,
    size: 2.0,           // px
    sizeAttenuation: false,
    transparent: true,
    opacity: 0.35, // Increased to make shimmer more visible
    blending: THREE.AdditiveBlending
  });
  const vertices = new THREE.Points(ptsGeo, ptsMat);
  scene.add(vertices);

  // --- Animate iridescent shimmer along u and v ---
  const waves = 3;          // number of wave fronts
  const sigma = 0.025;      // width of each wave
  const speed = 0.08;       // revolutions per second (reduced by 1/3 from 0.12)
  const baseGray = 0.50;    // base brightness
  
  // Iridescent shimmer controls
  const hueA = 290/360;     // purple
  const hueB = 140/360;     // green
  const microScales = 8;    // reduced density for more visible effect
  const microSpeed = 0.8;   // faster animation to make it more obvious
  
  function brightnessAt(u, t){
    let b = 0.0;
    for(let k=0;k<waves;k++){
      const center = (k / waves + speed * t) % 1;
      let d = Math.abs(u - center);
      d = Math.min(d, 1 - d); // circular distance
      b += Math.exp(-0.5 * (d*d) / (sigma*sigma));
    }
    // normalize gently; cap at 1
    return Math.min(1.0, b);
  }
  
  // Utilities
  function clamp(x,a,b){ return Math.max(a, Math.min(b, x)); }
  function hsl2rgb(h, s, l){ // h in [0,1]
    const k = n => (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    const f = n => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    return [f(0), f(8), f(4)];
  }
  
  function iridescentRGB(u, v, t, seed){
    const wave = brightnessAt(u, t);                     // existing longitudinal pulse (0..1)
    const phase = 2*Math.PI*(v*microScales + microSpeed*t) + seed*6.283;
    const mixH = 0.5 + 0.5*Math.sin(phase);             // 0..1, animated along v
    const h = hueA*(1-mixH) + hueB*mixH;
    
    // Make inactive areas much dimmer
    const baseOpacity = 0.075; // 40% less opaque (0.125 * 0.6)
    const activeBoost = 0.925; // increased boost to maintain bright shimmer
    const s = (0.4 + 0.3*wave) * 0.89;                 // increased saturation by 33% (0.67 * 1.33 = 0.89)
    const l = clamp(baseOpacity + activeBoost*wave, 0, 1); // much dimmer base, bright on pulse
    return hsl2rgb(h, s, l);
  }

  // Store original positions for wobble animation
  const originalLinePositions = new Float32Array(linePositions);
  const originalPointsPositions = new Float32Array(pointsPositions);
  
  // Wobble parameters for protein-like undulation
  const wobbleAmplitude = 0.075; // subtle organic deformation
  const wobbleSpeed = 0.5;        // half the speed too
  const wobbleComplexity = 3;     // multiple frequency layers
  
  const clock = new THREE.Clock();
  function tick(){
    const t = clock.getElapsedTime();

    // Apply gentle wobble to vertices (protein-like undulation)
    const linePos = lineGeo.getAttribute('position');
    for(let i = 0; i < lineUs.length; i++){
      const baseX = originalLinePositions[i*3+0];
      const baseY = originalLinePositions[i*3+1]; 
      const baseZ = originalLinePositions[i*3+2];
      
      // Multiple noise frequencies for organic motion using cylindrical wrapping
      const vAngle = lineVs[i] * 2 * Math.PI; // Convert v to angle (0 to 2π)
      const vCos = Math.cos(vAngle);
      const vSin = Math.sin(vAngle);
      
      // Use cylindrical coordinates for seamless wrapping
      const noise1 = Math.sin(t * wobbleSpeed + lineUs[i] * 8 + vCos * 6 + vSin * 4) * wobbleAmplitude;
      const noise2 = Math.sin(t * wobbleSpeed * 1.7 + lineUs[i] * 12 + vCos * 9 + vSin * 7) * wobbleAmplitude * 0.6;
      const noise3 = Math.sin(t * wobbleSpeed * 2.3 + lineUs[i] * 15 + vCos * 11 + vSin * 8) * wobbleAmplitude * 0.3;
      
      const wobble = noise1 + noise2 + noise3;
      
      // Apply wobble in normal direction (perpendicular to surface)
      linePos.array[i*3+0] = baseX + wobble * Math.cos(lineUs[i] * Math.PI * 4);
      linePos.array[i*3+1] = baseY + wobble * Math.sin(lineUs[i] * Math.PI * 4);
      linePos.array[i*3+2] = baseZ + wobble * Math.cos(lineVs[i] * Math.PI * 2);
    }
    linePos.needsUpdate = true;
    
    // Apply same wobble to points
    const pointPos = ptsGeo.getAttribute('position');
    for(let i = 0; i < pointsUs.length; i++){
      const baseX = originalPointsPositions[i*3+0];
      const baseY = originalPointsPositions[i*3+1];
      const baseZ = originalPointsPositions[i*3+2];
      
      const vAngle = pointsVs[i] * 2 * Math.PI; // Convert v to angle (0 to 2π)
      const vCos = Math.cos(vAngle);
      const vSin = Math.sin(vAngle);
      
      // Use cylindrical coordinates for seamless wrapping
      const noise1 = Math.sin(t * wobbleSpeed + pointsUs[i] * 8 + vCos * 6 + vSin * 4) * wobbleAmplitude;
      const noise2 = Math.sin(t * wobbleSpeed * 1.7 + pointsUs[i] * 12 + vCos * 9 + vSin * 7) * wobbleAmplitude * 0.6;
      const noise3 = Math.sin(t * wobbleSpeed * 2.3 + pointsUs[i] * 15 + vCos * 11 + vSin * 8) * wobbleAmplitude * 0.3;
      
      const wobble = noise1 + noise2 + noise3;
      
      pointPos.array[i*3+0] = baseX + wobble * Math.cos(pointsUs[i] * Math.PI * 4);
      pointPos.array[i*3+1] = baseY + wobble * Math.sin(pointsUs[i] * Math.PI * 4);
      pointPos.array[i*3+2] = baseZ + wobble * Math.cos(pointsVs[i] * Math.PI * 2);
    }
    pointPos.needsUpdate = true;

    // Rotate as a whole - Z-axis only (like a wheel spinning)
    edges.rotation.set(0, 0, t*0.23625); // Z-axis rotation only
    vertices.rotation.copy(edges.rotation);

    // Update line colors with iridescent shimmer
    const lc = lineGeo.getAttribute('color');
    for(let i=0;i<lineUs.length;i++){
      const rgb = iridescentRGB(lineUs[i], lineVs[i], t, lineSeed[i]);
      lc.array[i*3+0] = rgb[0];
      lc.array[i*3+1] = rgb[1];
      lc.array[i*3+2] = rgb[2];
    }
    lc.needsUpdate = true;

    // Update point colors with iridescent shimmer
    const pc = ptsGeo.getAttribute('color');
    for(let i=0;i<pointsUs.length;i++){
      const rgb = iridescentRGB(pointsUs[i], pointsVs[i], t, pointsSeed[i]);
      pc.array[i*3+0] = rgb[0];
      pc.array[i*3+1] = rgb[1];
      pc.array[i*3+2] = rgb[2];
    }
    pc.needsUpdate = true;

    renderer.render(scene, camera);
    requestAnimationFrame(tick);
  }

  // Initialize and start
  resizeRenderer();
  tick();

  // Resize handler
  window.addEventListener('resize', resizeRenderer);
});