import { CUSTOM_SHADERS, VANILLA_SHADERS } from "./balatro-utils";

export const SHADER_PREVIEW_WIDTH = 142;
export const SHADER_PREVIEW_HEIGHT = 190;

export interface ShaderPreviewImages {
  baseImage?: string;
  image?: string;
  overlayImage?: string;
}

export interface EditionShaderFrame {
  render: (canvas: HTMLCanvasElement, time: number) => void;
}

const VERTEX_SOURCE = `
attribute vec2 preview_position;
varying mediump vec2 preview_uv;
void main() {
  preview_uv = vec2((preview_position.x + 1.0) * 0.5, (1.0 - preview_position.y) * 0.5);
  gl_Position = vec4(preview_position, 0.0, 1.0);
}`;

export function adaptLoveFragmentShader(source: string): string {
  const fragment = source
    .replace(/#ifdef VERTEX\b[\s\S]*?#endif/g, "")
    .split("\n")
    .map((line) =>
      line.trimStart().startsWith("#")
        ? line
        : line.replace(/(?<![\w.])\d+(?![\w.])/g, "$&.0"),
    )
    .join("\n");

  return `
precision mediump float;
#define extern uniform
#define number float
#define Image sampler2D
#define Texel texture2D
uniform sampler2D preview_texture;
varying mediump vec2 preview_uv;
${fragment}
void main() {
  gl_FragColor = effect(vec4(1.0), preview_texture, preview_uv, gl_FragCoord.xy);
}`;
}

// Browser equivalents for the built-in effects
function vanillaFragmentShader(shader: string): string {
  const mode = VANILLA_SHADERS.findIndex((option) => option.key === shader);
  return `
precision mediump float;
uniform sampler2D preview_texture;
uniform float time;
varying mediump vec2 preview_uv;
vec3 rainbow(float phase) {
  return 0.5 + 0.5 * cos(6.2831853 * (phase + vec3(0.0, 0.33333, 0.66667)));
}
void main() {
  vec2 uv = preview_uv;
  vec4 pixel = texture2D(preview_texture, uv);
  float brightness = dot(pixel.rgb, vec3(0.2126, 0.7152, 0.0722));
  float wave = 0.5 + 0.5 * sin(uv.x * 19.0 + uv.y * 12.0 + time * 1.8);
  float gleam = pow(wave, 8.0);
  vec3 spectrum = rainbow(uv.x * 0.65 + uv.y * 0.45 + time * 0.08);
  if (${mode} == 0) {
    float grain = 0.5 + 0.5 * sin(uv.x * 220.0 + sin(uv.y * 190.0));
    pixel.rgb = mix(pixel.rgb, vec3(brightness) * vec3(0.68, 0.83, 1.08), 0.35);
    pixel.rgb += (grain * 0.07 + gleam * 0.32) * vec3(0.7, 0.85, 1.0);
  } else if (${mode} == 1) {
    float rings = sin(length((uv - 0.5) * vec2(1.0, 1.34)) * 65.0 - time * 1.8);
    pixel.rgb = mix(pixel.rgb, pixel.rgb * (0.65 + spectrum * 1.05), 0.65);
    pixel.rgb += (rings * 0.05 + gleam * 0.13) * spectrum;
  } else if (${mode} == 2) {
    pixel.rgb *= 0.55 + 1.1 * spectrum;
    pixel.rgb += gleam * 0.10;
  } else if (${mode} == 3) {
    pixel.rgb += gleam * vec3(0.35, 0.28, 0.12);
    pixel.rgb *= 0.92 + 0.15 * wave;
  } else if (${mode} == 4) {
    pixel.rgb = vec3(brightness * 0.6);
  } else if (${mode} == 5) {
    pixel.rgb += gleam * vec3(0.3, 0.22, 0.08);
  } else if (${mode} == 6 || ${mode} == 7) {
    pixel.rgb = vec3(1.0) - pixel.rgb;
    pixel.rgb = mix(pixel.rgb, vec3(dot(pixel.rgb, vec3(0.33333))), 0.08);
    if (${mode} == 7) pixel.rgb += gleam * vec3(0.2, 0.15, 0.25);
  }
  gl_FragColor = vec4(clamp(pixel.rgb, 0.0, 1.0), pixel.a);
}`;
}

const sourceCache = new Map<string, Promise<string>>();

function shaderSource(shader: string): Promise<string> {
  const cached = sourceCache.get(shader);
  if (cached) return cached;

  const custom = CUSTOM_SHADERS.find((option) => option.key === shader);
  const source = custom
    ? fetch(custom.filepath).then(async (response) => {
        if (!response.ok) throw new Error("Shader source unavailable");
        return adaptLoveFragmentShader(await response.text());
      })
    : VANILLA_SHADERS.some((option) => option.key === shader)
      ? Promise.resolve(vanillaFragmentShader(shader))
      : Promise.reject(new Error("Shader preview unavailable"));
  sourceCache.set(shader, source);
  return source;
}

// Cache only app assets
const imageCache = new Map<string, Promise<HTMLImageElement>>();

function loadImage(source: string): Promise<HTMLImageElement> {
  const cached = imageCache.get(source);
  if (cached) return cached;
  const result = new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Preview image unavailable"));
    image.src = source;
  });
  if (source.startsWith("/")) {
    if (imageCache.size >= 32) imageCache.delete(imageCache.keys().next().value!);
    imageCache.set(source, result);
  }
  return result;
}

async function composeImages(images: ShaderPreviewImages): Promise<HTMLCanvasElement> {
  const sources = [images.baseImage, images.image, images.overlayImage].filter(
    (source): source is string => Boolean(source),
  );
  const loaded = await Promise.allSettled(sources.map(loadImage));
  const surface = document.createElement("canvas");
  surface.width = SHADER_PREVIEW_WIDTH;
  surface.height = SHADER_PREVIEW_HEIGHT;
  const context = surface.getContext("2d");
  if (!context) throw new Error("Canvas preview unavailable");
  context.imageSmoothingEnabled = false;
  let drawn = false;
  for (const result of loaded) {
    if (result.status !== "fulfilled") continue;
    const image = result.value;
    const scale = Math.min(surface.width / image.width, surface.height / image.height);
    const width = image.width * scale;
    const height = image.height * scale;
    context.drawImage(image, (surface.width - width) / 2, (surface.height - height) / 2, width, height);
    drawn = true;
  }
  if (!drawn) throw new Error("Preview image unavailable");
  return surface;
}

interface PreviewProgram {
  program: WebGLProgram;
  position: number;
  uniforms: Map<string, WebGLUniformLocation>;
}

class SharedShaderRenderer {
  private readonly canvas = document.createElement("canvas");
  private readonly gl: WebGLRenderingContext;
  private readonly buffer: WebGLBuffer;
  private readonly texture: WebGLTexture;
  private readonly programs = new Map<string, PreviewProgram>();
  private lastImage?: HTMLCanvasElement;

  constructor() {
    this.canvas.width = SHADER_PREVIEW_WIDTH;
    this.canvas.height = SHADER_PREVIEW_HEIGHT;
    const gl = this.canvas.getContext("webgl", {
      alpha: true,
      premultipliedAlpha: false,
      antialias: false,
      preserveDrawingBuffer: false,
      powerPreference: "low-power",
    });
    if (!gl) throw new Error("WebGL preview unavailable");
    this.gl = gl;
    const buffer = gl.createBuffer();
    const texture = gl.createTexture();
    if (!buffer || !texture) throw new Error("WebGL preview unavailable");
    this.buffer = buffer;
    this.texture = texture;
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  }

  private compile(type: number, source: string): WebGLShader {
    const gl = this.gl;
    const shader = gl.createShader(type);
    if (!shader) throw new Error("Shader preview unavailable");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const message = gl.getShaderInfoLog(shader) || "Shader compilation failed";
      gl.deleteShader(shader);
      throw new Error(message);
    }
    return shader;
  }

  prepare(shader: string, source: string): void {
    const gl = this.gl;
    if (gl.isContextLost()) throw new Error("WebGL preview unavailable");
    if (this.programs.has(shader)) return;
    const vertex = this.compile(gl.VERTEX_SHADER, VERTEX_SOURCE);
    let fragment: WebGLShader | undefined;
    let program: WebGLProgram | null = null;
    try {
      fragment = this.compile(gl.FRAGMENT_SHADER, source);
      program = gl.createProgram();
      if (!program) throw new Error("Shader preview unavailable");
      gl.attachShader(program, vertex);
      gl.attachShader(program, fragment);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        throw new Error(gl.getProgramInfoLog(program) || "Shader linking failed");
      }
      const uniforms = new Map<string, WebGLUniformLocation>();
      const uniformCount = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS) as number;
      for (let index = 0; index < uniformCount; index++) {
        const info = gl.getActiveUniform(program, index);
        if (!info) continue;
        const location = gl.getUniformLocation(program, info.name);
        if (location !== null) uniforms.set(info.name, location);
      }
      this.programs.set(shader, {
        program,
        position: gl.getAttribLocation(program, "preview_position"),
        uniforms,
      });
    } catch (error) {
      if (program) gl.deleteProgram(program);
      throw error;
    } finally {
      gl.deleteShader(vertex);
      if (fragment) gl.deleteShader(fragment);
    }
  }

  render(shader: string, image: HTMLCanvasElement, target: HTMLCanvasElement, time: number): void {
    const gl = this.gl;
    const entry = this.programs.get(shader);
    const output = target.getContext("2d");
    if (!entry || !output || gl.isContextLost()) throw new Error("Shader preview unavailable");
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.useProgram(entry.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(entry.position);
    gl.vertexAttribPointer(entry.position, 2, gl.FLOAT, false, 0, 0);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    if (this.lastImage !== image) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
      this.lastImage = image;
    }
    for (const [name, location] of entry.uniforms) {
      if (name === "preview_texture" || name === "shadow") gl.uniform1i(location, 0);
      else if (name === "texture_details") gl.uniform4f(location, 0, 0, image.width, image.height);
      else if (name === "image_details") gl.uniform2f(location, image.width, image.height);
      else if (name === shader) gl.uniform2f(location, 1.0 + Math.sin(time / 2) * 0.6, time);
      else if (name === "time") gl.uniform1f(location, time);
      else if (name === "lines_offset") gl.uniform1f(location, 0.35);
      else if (name.startsWith("burn_colour")) gl.uniform4f(location, 0, 0, 0, 0);
      else if (name === "mouse_screen_pos") gl.uniform2f(location, 0, 0);
      else gl.uniform1f(location, name === "screen_scale" ? 1 : 0);
    }
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    if (target.width !== image.width) target.width = image.width;
    if (target.height !== image.height) target.height = image.height;
    output.clearRect(0, 0, target.width, target.height);
    output.drawImage(this.canvas, 0, 0);
  }
}

let sharedRenderer: SharedShaderRenderer | undefined;

export async function prepareEditionShaderPreview(
  shader: string,
  images: ShaderPreviewImages,
): Promise<EditionShaderFrame> {
  const [source, image] = await Promise.all([shaderSource(shader), composeImages(images)]);
  sharedRenderer ??= new SharedShaderRenderer();
  const renderer = sharedRenderer;
  renderer.prepare(shader, source);
  return { render: (canvas, time) => renderer.render(shader, image, canvas, time) };
}
