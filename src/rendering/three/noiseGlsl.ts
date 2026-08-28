/**
 * The value-noise kit every procedural surface in the world shares.
 *
 * One copy, because two would drift: the landform bands and the laid paving are drawn by different
 * materials but must agree about what "the same point in the world" looks like, or a yard laid over
 * a hillside would ripple against the hillside it is lying on. Every function here takes world-space
 * metres and nothing else — no UV, no instance index, no axial coordinate — which is what keeps a
 * pattern from re-drawing the hex lattice the prisms already carry.
 *
 * `hfFbm` needs an `HF_OCTAVES` define. Every material that injects this sets one.
 */
export const NOISE_GLSL = /* glsl */ `
float hfHash12( vec2 p ) {
	vec3 q = fract( vec3( p.xyx ) * 0.1031 );
	q += dot( q, q.yzx + 33.33 );
	return fract( ( q.x + q.y ) * q.z );
}

float hfValue( vec2 p ) {
	vec2 cell = floor( p );
	vec2 f = fract( p );
	vec2 u = f * f * ( 3.0 - 2.0 * f );
	return mix(
		mix( hfHash12( cell ), hfHash12( cell + vec2( 1.0, 0.0 ) ), u.x ),
		mix( hfHash12( cell + vec2( 0.0, 1.0 ) ), hfHash12( cell + vec2( 1.0, 1.0 ) ), u.x ),
		u.y
	);
}

float hfFbm( vec2 p ) {
	float sum = 0.0;
	float weight = 0.0;
	float amplitude = 0.55;
	for ( int octave = 0; octave < HF_OCTAVES; octave += 1 ) {
		sum += hfValue( p ) * amplitude;
		weight += amplitude;
		p = p * 2.07 + 19.19;
		amplitude *= 0.5;
	}
	return sum / weight;
}

/** 1 inside a unit interval, falling to 0 within \`width\` of either end. Joints and seams. */
float hfEdge( float t, float width ) {
	return smoothstep( 0.0, width, t ) * ( 1.0 - smoothstep( 1.0 - width, 1.0, t ) );
}
`;
