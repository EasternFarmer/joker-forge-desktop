import { useEffect, useMemo, useRef, useState } from "react";
import {
  EditionShaderFrame,
  prepareEditionShaderPreview,
  SHADER_PREVIEW_HEIGHT,
  SHADER_PREVIEW_WIDTH,
} from "@/lib/balatro/shader-preview";
import { cn } from "@/lib/core/utils";

interface EditionShaderPreviewProps {
  shader?: string | false;
  baseImage?: string;
  image?: string;
  overlayImage?: string;
  disableBaseShader?: boolean;
  animate?: boolean;
  className?: string;
  alt?: string;
}

export function EditionShaderPreview({
  shader,
  baseImage,
  image,
  overlayImage,
  disableBaseShader = false,
  animate = true,
  className,
  alt = "Edition shader preview",
}: EditionShaderPreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const shaderValue = typeof shader === "string" ? shader.trim() : "";
  const shaderKey = shaderValue === "false" ? "" : shaderValue;
  const input = useMemo(
    () => ({ shader: shaderKey, baseImage, image, overlayImage }),
    [shaderKey, baseImage, image, overlayImage],
  );
  const [preview, setPreview] = useState<{
    input: typeof input;
    status: "loading" | "ready" | "unavailable";
    frame?: EditionShaderFrame;
  }>({ input, status: "loading" });
  const ready = preview.input === input && preview.status === "ready";
  const unavailable = preview.input === input && preview.status === "unavailable";

  useEffect(() => {
    let cancelled = false;
    setPreview({ input, status: "loading" });
    if (!input.shader) return;
    prepareEditionShaderPreview(input.shader, input)
      .then((frame) => {
        if (cancelled || !canvasRef.current) return;
        frame.render(canvasRef.current, 2);
        setPreview({ input, status: "ready", frame });
      })
      .catch(() => {
        if (!cancelled) setPreview({ input, status: "unavailable" });
      });
    return () => { cancelled = true; };
  }, [input]);

  useEffect(() => {
    if (!animate || !ready || !preview.frame) return;
    const frame = preview.frame;
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let visible = typeof IntersectionObserver === "undefined";
    let request = 0;
    let lastTime = 0;
    let elapsed = 2;
    let stopped = false;

    const tick = (timestamp: number) => {
      if (stopped) return;
      const delta = lastTime ? timestamp - lastTime : 0;
      if (!lastTime || delta >= 1000 / 30) {
        elapsed += delta / 1000;
        lastTime = timestamp;
        try {
          if (canvasRef.current) frame.render(canvasRef.current, elapsed);
        } catch {
          stopped = true;
          setPreview({ input, status: "unavailable" });
          return;
        }
      }
      request = requestAnimationFrame(tick);
    };
    const updateAnimation = () => {
      cancelAnimationFrame(request);
      lastTime = 0;
      if (!stopped && visible && !document.hidden && !motion.matches) {
        request = requestAnimationFrame(tick);
      }
    };
    const observer = typeof IntersectionObserver === "undefined" ? undefined : new IntersectionObserver(
      ([entry]) => { visible = entry.isIntersecting; updateAnimation(); },
    );
    if (containerRef.current) observer?.observe(containerRef.current);
    document.addEventListener("visibilitychange", updateAnimation);
    motion.addEventListener("change", updateAnimation);
    updateAnimation();
    return () => {
      stopped = true;
      cancelAnimationFrame(request);
      observer?.disconnect();
      document.removeEventListener("visibilitychange", updateAnimation);
      motion.removeEventListener("change", updateAnimation);
    };
  }, [animate, input, preview.frame, ready]);

  return (
    <div
      ref={containerRef}
      className={cn("relative w-full h-full pointer-events-none", className)}
      role="img"
      aria-label={alt}
      data-shader={shaderKey || "none"}
      data-preview-state={!shaderKey ? "none" : ready ? "ready" : unavailable ? "unavailable" : "loading"}
    >
      <div className={cn("absolute inset-0", ready && disableBaseShader && "invisible")} aria-hidden="true">
        {[baseImage, image, overlayImage].map((source, index) => source && (
          <img
            key={`${index}:${source}`}
            src={source}
            alt=""
            draggable="false"
            onError={(event) => { event.currentTarget.style.visibility = "hidden"; }}
            className="absolute inset-0 w-full h-full object-contain [image-rendering:pixelated]"
          />
        ))}
      </div>
      <canvas
        ref={canvasRef}
        width={SHADER_PREVIEW_WIDTH}
        height={SHADER_PREVIEW_HEIGHT}
        aria-hidden="true"
        className={cn("absolute inset-0 w-full h-full object-contain [image-rendering:pixelated]", !ready && "invisible")}
      />
      {unavailable && (
        <span className="absolute inset-x-1 bottom-1 rounded bg-black/70 px-1 py-0.5 text-center text-[10px] leading-tight text-white">
          Preview unavailable
        </span>
      )}
    </div>
  );
}
