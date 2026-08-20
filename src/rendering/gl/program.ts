export function compileProgram(
  gl: WebGL2RenderingContext,
  vertexSource: string,
  fragmentSource: string,
  label: string,
): WebGLProgram {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, vertexSource, label);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource, label);
  const program = gl.createProgram();
  if (!program) throw new Error(`${label}: program`);
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program) ?? "link failed";
    gl.deleteProgram(program);
    throw new Error(`${label}: ${log}`);
  }
  return program;
}

function compileShader(
  gl: WebGL2RenderingContext,
  type: number,
  source: string,
  label: string,
): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error(`${label}: shader`);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "compile failed";
    gl.deleteShader(shader);
    throw new Error(`${label}: ${log}`);
  }
  return shader;
}

export function createBuffer(gl: WebGL2RenderingContext): WebGLBuffer {
  const buffer = gl.createBuffer();
  if (!buffer) throw new Error("WebGL buffer");
  return buffer;
}

export function createTexture(gl: WebGL2RenderingContext): WebGLTexture {
  const texture = gl.createTexture();
  if (!texture) throw new Error("WebGL texture");
  return texture;
}

export function createVertexArray(
  gl: WebGL2RenderingContext,
): WebGLVertexArrayObject {
  const vao = gl.createVertexArray();
  if (!vao) throw new Error("WebGL vertex array");
  return vao;
}

/** Cached uniform locations so a draw does not query the driver by name. */
export class Uniforms {
  private readonly cache = new Map<string, WebGLUniformLocation | null>();

  constructor(
    private readonly gl: WebGL2RenderingContext,
    private readonly program: WebGLProgram,
  ) {}

  loc(name: string): WebGLUniformLocation | null {
    let location = this.cache.get(name);
    if (location === undefined) {
      location = this.gl.getUniformLocation(this.program, name);
      this.cache.set(name, location);
    }
    return location;
  }

  vec2(name: string, value: readonly [number, number]): void {
    this.gl.uniform2f(this.loc(name), value[0], value[1]);
  }

  vec4(name: string, value: readonly [number, number, number, number]): void {
    this.gl.uniform4f(this.loc(name), value[0], value[1], value[2], value[3]);
  }

  f(name: string, value: number): void {
    this.gl.uniform1f(this.loc(name), value);
  }

  i(name: string, value: number): void {
    this.gl.uniform1i(this.loc(name), value);
  }
}

/** Unit quad covering [-1, 1] in both axes, two triangles. */
export const QUAD = new Float32Array([
  -1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1,
]);
