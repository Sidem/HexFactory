/**
 * Instanced hex fill/stroke/hatch, plus an optional terrain-tile sample. Pointy-top, Y-down,
 * matching `hexDraw` — circumradius 1 in `v_local`.
 */
export const HEX_VS = `#version 300 es
layout(location=0) in vec2 a_corner;
layout(location=1) in vec2 a_world;
layout(location=2) in vec4 a_fill;
layout(location=3) in vec4 a_stroke;
layout(location=4) in vec4 a_misc;
layout(location=5) in vec4 a_extra;
uniform vec2 u_origin;
uniform vec2 u_resolution;
uniform float u_scale;
uniform float u_hex;
out vec2 v_local;
out vec4 v_fill;
out vec4 v_stroke;
out vec4 v_misc;
out vec4 v_extra;
void main() {
  float size = u_hex * a_misc.x;
  vec2 screen = u_origin + a_world * u_scale + a_corner * size;
  vec2 clip = screen / u_resolution * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_local = a_corner;
  v_fill = a_fill;
  v_stroke = a_stroke;
  v_misc = a_misc;
  v_extra = a_extra;
}
`;

export const HEX_FS = `#version 300 es
precision highp float;
precision highp sampler2DArray;
in vec2 v_local;
in vec4 v_fill;
in vec4 v_stroke;
in vec4 v_misc;
in vec4 v_extra;
uniform float u_time;
uniform sampler2DArray u_tiles;
layout(location=0) out vec4 outColor;

float hexDist(vec2 p) {
  p = abs(p);
  return max(p.x, p.x * 0.5 + p.y * 0.86602540378);
}

void main() {
  float d = hexDist(v_local);
  float edge = 0.86602540378;
  float fw = fwidth(d) * 1.25;
  float inside = 1.0 - smoothstep(edge - fw, edge + fw, d);
  if (inside < 0.02) discard;

  vec4 color = v_fill;
  float layer = v_misc.z;
  if (layer > 0.5) {
    float c = cos(v_misc.y);
    float s = sin(v_misc.y);
    vec2 rotated = vec2(
      c * v_local.x - s * v_local.y,
      s * v_local.x + c * v_local.y
    );
    vec2 uv = rotated * 0.5 + 0.5;
    vec4 tex = texture(u_tiles, vec3(uv, layer - 1.0));
    color = mix(color, vec4(tex.rgb, max(color.a, tex.a)), tex.a);
  }
  float jitter = v_extra.x;
  color.rgb += vec3(jitter) * (jitter > 0.0 ? 0.07 : 0.09);

  float water = v_extra.w;
  if (water > 0.5) {
    float phase = u_time / 680.0 + v_extra.y * 0.0002;
    float wave = 0.5 + 0.5 * sin(v_local.y * 6.0 + phase);
    color.rgb = mix(color.rgb, vec3(0.75, 0.89, 1.0), 0.10 + 0.08 * wave);
  }

  float hatch = v_extra.z;
  if (hatch > 0.5) {
    float stripe = fract((v_local.x + v_local.y) * 3.4);
    if (stripe > 0.55 && stripe < 0.78) {
      color.rgb = mix(color.rgb, v_stroke.rgb, 0.35);
    }
  }

  float strokeW = max(fw * 1.6, 0.04 * v_misc.x);
  float rim = smoothstep(edge - strokeW - fw, edge - strokeW + fw, d);
  color = mix(color, v_stroke, rim * v_stroke.a);

  if (v_misc.w > 0.5) {
    float p = 0.03 * sin(u_time / 450.0);
    color.a *= 0.92 + p;
  }

  color.a *= inside;
  outColor = color;
}
`;

export const SPRITE_VS = `#version 300 es
layout(location=0) in vec2 a_corner;
layout(location=1) in vec2 a_world;
layout(location=2) in vec4 a_uv;
layout(location=3) in vec4 a_tint;
layout(location=4) in vec2 a_dim;
uniform vec2 u_origin;
uniform vec2 u_resolution;
uniform float u_scale;
uniform float u_hex;
out vec2 v_uv;
out vec4 v_tint;
void main() {
  vec2 corner = a_corner * a_dim * u_hex * 0.5;
  vec2 screen = u_origin + a_world * u_scale + corner;
  vec2 clip = screen / u_resolution * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_uv = mix(a_uv.xy, a_uv.zw, a_corner * 0.5 + 0.5);
  v_tint = a_tint;
}
`;

export const SPRITE_FS = `#version 300 es
precision highp float;
in vec2 v_uv;
in vec4 v_tint;
uniform sampler2D u_atlas;
layout(location=0) out vec4 outColor;
void main() {
  vec4 tex = texture(u_atlas, v_uv);
  vec4 color = tex * v_tint;
  if (color.a < 0.04) discard;
  outColor = color;
}
`;

export const CHUNK_VS = `#version 300 es
layout(location=0) in vec2 a_corner;
layout(location=1) in vec2 a_world;
layout(location=2) in float a_span;
uniform vec2 u_origin;
uniform vec2 u_resolution;
uniform float u_scale;
out vec2 v_uv;
void main() {
  vec2 local = (a_corner * 0.5 + 0.5) * a_span;
  vec2 screen = u_origin + (a_world + local) * u_scale;
  vec2 clip = screen / u_resolution * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_uv = a_corner * 0.5 + 0.5;
}
`;

export const CHUNK_FS = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_field;
uniform vec4 u_fill;
layout(location=0) out vec4 outColor;
void main() {
  vec4 tex = texture(u_field, v_uv * 4.0);
  outColor = mix(u_fill, tex, 0.28);
}
`;

export const FOG_VS = `#version 300 es
layout(location=0) in vec2 a_corner;
out vec2 v_uv;
void main() {
  v_uv = a_corner * 0.5 + 0.5;
  gl_Position = vec4(a_corner, 0.0, 1.0);
}
`;

/**
 * Axis-aligned (or rotated) coloured quads in player-centred world space. The minimap's primitive
 * is a rectangle, matching the 2D map it replaced.
 */
export const MAP_VS = `#version 300 es
layout(location=0) in vec2 a_corner;
layout(location=1) in vec2 a_world;
layout(location=2) in vec2 a_extent;
layout(location=3) in vec4 a_color;
layout(location=4) in vec2 a_spin;
uniform vec2 u_player;
uniform vec2 u_resolution;
uniform float u_scale;
out vec2 v_corner;
out vec4 v_color;
out float v_shape;
void main() {
  float c = cos(a_spin.x);
  float s = sin(a_spin.x);
  vec2 local = a_corner * a_extent;
  vec2 rotated = vec2(c * local.x - s * local.y, s * local.x + c * local.y);
  vec2 screen = u_resolution * 0.5 + (a_world - u_player) * u_scale + rotated * u_scale;
  vec2 clip = screen / u_resolution * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_corner = a_corner;
  v_color = a_color;
  v_shape = a_spin.y;
}
`;

export const MAP_FS = `#version 300 es
precision highp float;
in vec2 v_corner;
in vec4 v_color;
in float v_shape;
layout(location=0) out vec4 outColor;
void main() {
  if (v_shape > 0.5) {
    if (dot(v_corner, v_corner) > 1.0) discard;
  }
  outColor = v_color;
}
`;

export const FOG_FS = `#version 300 es
precision highp float;
in vec2 v_uv;
uniform vec2 u_origin;
uniform vec2 u_resolution;
uniform float u_scale;
uniform vec2 u_worldMin;
uniform vec2 u_worldSpan;
uniform sampler2D u_coverage;
uniform vec4 u_fog;
layout(location=0) out vec4 outColor;
void main() {
  vec2 screen = vec2(gl_FragCoord.x, u_resolution.y - gl_FragCoord.y);
  vec2 world = (screen - u_origin) / max(u_scale, 0.0001);
  vec2 uv = (world - u_worldMin) / max(u_worldSpan, vec2(1.0));
  float covered = 0.0;
  if (uv.x >= 0.0 && uv.y >= 0.0 && uv.x <= 1.0 && uv.y <= 1.0) {
    covered = texture(u_coverage, uv).r;
  }
  float fog = 1.0 - smoothstep(0.08, 0.55, covered);
  float hatch = step(0.55, fract((screen.x + screen.y) / 26.0));
  vec3 tone = mix(u_fog.rgb, u_fog.rgb * 1.08, hatch * 0.35);
  outColor = vec4(tone, fog * u_fog.a);
}
`;
