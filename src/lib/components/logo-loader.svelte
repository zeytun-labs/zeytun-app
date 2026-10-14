<script lang="ts">
  export type LogoLoaderVariant =
    | "shimmer"
    | "stroke"
    | "aperture"
    | "radar"
    | "orbit";

  type Props = {
    size?: number;
    variant?: LogoLoaderVariant;
    class?: string;
  };

  let {
    size = 56,
    variant = "shimmer",
    class: className,
  }: Props = $props();

  const fullPath =
    "M0 21.4821C10.2275 21.4821 18.5187 29.7726 18.5189 40C8.29146 40 0.000269965 31.7095 0 21.4821ZM40 21.4821C39.9997 31.7093 31.7093 39.9998 21.4821 40C21.4823 29.7727 29.7727 21.4823 40 21.4821ZM18.5189 0C18.5189 10.2276 10.2276 18.5189 0 18.5189C0 8.2913 8.2913 0 18.5189 0ZM21.4821 0C31.7095 0.000236235 40 8.29144 40 18.5189C29.7726 18.5187 21.4821 10.2275 21.4821 0Z";

  const tlPath =
    "M18.5189 0C18.5189 10.2276 10.2276 18.5189 0 18.5189C0 8.2913 8.2913 0 18.5189 0Z";
  const trPath =
    "M21.4821 0C31.7095 0.000236235 40 8.29144 40 18.5189C29.7726 18.5187 21.4821 10.2275 21.4821 0Z";
  const brPath =
    "M40 21.4821C39.9997 31.7093 31.7093 39.9998 21.4821 40C21.4823 29.7727 29.7727 21.4823 40 21.4821Z";
  const blPath =
    "M0 21.4821C10.2275 21.4821 18.5187 29.7726 18.5189 40C8.29146 40 0.000269965 31.7095 0 21.4821Z";
</script>

<div
  class="relative inline-flex items-center justify-center select-none {className ?? ''}"
  style="width: {size}px; height: {size}px;"
>
  {#if variant === "shimmer"}
    <!-- 1. SHIMMER / SHEEN SWEEP -->
    <div class="relative flex items-center justify-center" style="width: {size}px; height: {size}px;">
      <svg
        width={size}
        height={size}
        viewBox="0 0 40 40"
        fill="currentColor"
        class="text-primary drop-shadow-sm"
      >
        <path d={fullPath} />
      </svg>
      <div class="sheen-overlay" style="width: {size}px; height: {size}px;">
        <div class="sheen-beam"></div>
      </div>
    </div>
  {:else if variant === "stroke"}
    <!-- 2. STROKE DRAW & FILL -->
    <svg
      width={size}
      height={size}
      viewBox="0 0 40 40"
      fill="currentColor"
      class="text-primary drop-shadow-sm"
    >
      <path class="stroke-path" d={tlPath} />
      <path class="stroke-path" d={trPath} />
      <path class="stroke-path" d={brPath} />
      <path class="stroke-path" d={blPath} />
    </svg>
  {:else if variant === "aperture"}
    <!-- 3. APERTURE / IRIS SNAP -->
    <svg
      width={size}
      height={size}
      viewBox="0 0 40 40"
      fill="currentColor"
      class="text-primary drop-shadow-sm overflow-visible"
    >
      <path class="iris-tl" d={tlPath} />
      <path class="iris-tr" d={trPath} />
      <path class="iris-br" d={brPath} />
      <path class="iris-bl" d={blPath} />
    </svg>
  {:else if variant === "radar"}
    <!-- 4. RADAR ECHO -->
    <div class="relative flex items-center justify-center" style="width: {size}px; height: {size}px;">
      <svg
        class="echo-ring text-primary"
        width={size}
        height={size}
        viewBox="0 0 40 40"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
      >
        <path d={fullPath} />
      </svg>
      <svg
        class="echo-ring echo-ring-delayed text-primary"
        width={size}
        height={size}
        viewBox="0 0 40 40"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
      >
        <path d={fullPath} />
      </svg>
      <svg
        class="text-primary relative z-10 drop-shadow-sm"
        width={size}
        height={size}
        viewBox="0 0 40 40"
        fill="currentColor"
      >
        <path d={fullPath} />
      </svg>
    </div>
  {:else if variant === "orbit"}
    <!-- 5. STEPPED FLUID ORBIT -->
    <svg
      width={size}
      height={size}
      viewBox="0 0 40 40"
      fill="currentColor"
      class="orbit-svg text-primary drop-shadow-sm"
    >
      <path d={fullPath} />
    </svg>
  {/if}
</div>

<style>
  /* --- 1. Shimmer --- */
  .sheen-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    mix-blend-mode: screen;
    mask-image: url('data:image/svg+xml;utf8,<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg"><path d="M0 21.4821C10.2275 21.4821 18.5187 29.7726 18.5189 40C8.29146 40 0.000269965 31.7095 0 21.4821ZM40 21.4821C39.9997 31.7093 31.7093 39.9998 21.4821 40C21.4823 29.7727 29.7727 21.4823 40 21.4821ZM18.5189 0C18.5189 10.2276 10.2276 18.5189 0 18.5189C0 8.2913 8.2913 0 18.5189 0ZM21.4821 0C31.7095 0.000236235 40 8.29144 40 18.5189C29.7726 18.5187 21.4821 10.2275 21.4821 0Z" fill="black"/></svg>');
    mask-size: 100% 100%;
    -webkit-mask-image: url('data:image/svg+xml;utf8,<svg viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg"><path d="M0 21.4821C10.2275 21.4821 18.5187 29.7726 18.5189 40C8.29146 40 0.000269965 31.7095 0 21.4821ZM40 21.4821C39.9997 31.7093 31.7093 39.9998 21.4821 40C21.4823 29.7727 29.7727 21.4823 40 21.4821ZM18.5189 0C18.5189 10.2276 10.2276 18.5189 0 18.5189C0 8.2913 8.2913 0 18.5189 0ZM21.4821 0C31.7095 0.000236235 40 8.29144 40 18.5189C29.7726 18.5187 21.4821 10.2275 21.4821 0Z" fill="black"/></svg>');
    -webkit-mask-size: 100% 100%;
    overflow: hidden;
  }

  .sheen-beam {
    position: absolute;
    top: -60%;
    left: -60%;
    width: 220%;
    height: 220%;
    background: linear-gradient(
      90deg,
      transparent 20%,
      rgba(255, 255, 255, 0.05) 35%,
      rgba(255, 255, 255, 0.35) 48%,
      rgba(255, 255, 255, 0.45) 50%,
      rgba(255, 255, 255, 0.35) 52%,
      rgba(255, 255, 255, 0.05) 65%,
      transparent 80%
    );
    animation: sheenMove 3.8s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }

  @keyframes sheenMove {
    0% {
      transform: translateX(-160%) rotate(35deg);
    }
    38% {
      transform: translateX(180%) rotate(35deg);
    }
    100% {
      transform: translateX(180%) rotate(35deg);
    }
  }

  /* --- 2. Stroke Draw & Fill --- */
  .stroke-path {
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-dasharray: 70;
    animation: strokeDraw 2.4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }

  @keyframes strokeDraw {
    0% {
      stroke-dashoffset: 70;
      fill-opacity: 0;
    }
    45% {
      stroke-dashoffset: 0;
      fill-opacity: 0;
    }
    65% {
      stroke-dashoffset: 0;
      fill-opacity: 1;
    }
    85%,
    100% {
      stroke-dashoffset: 0;
      fill-opacity: 0.15;
    }
  }

  /* --- 3. Aperture / Iris Snap --- */
  @keyframes irisSnap {
    0%,
    100% {
      transform: translate(0, 0) rotate(0deg);
    }
    40% {
      transform: translate(var(--dx), var(--dy)) rotate(var(--rot));
    }
    75% {
      transform: translate(calc(var(--dx) * -0.2), calc(var(--dy) * -0.2))
        rotate(0deg);
    }
  }

  .iris-tl {
    --dx: -3px;
    --dy: -3px;
    --rot: -8deg;
    transform-box: fill-box;
    transform-origin: center;
    animation: irisSnap 1.8s cubic-bezier(0.34, 1.56, 0.64, 1) infinite;
  }
  .iris-tr {
    --dx: 3px;
    --dy: -3px;
    --rot: 8deg;
    transform-box: fill-box;
    transform-origin: center;
    animation: irisSnap 1.8s cubic-bezier(0.34, 1.56, 0.64, 1) infinite;
  }
  .iris-br {
    --dx: 3px;
    --dy: 3px;
    --rot: -8deg;
    transform-box: fill-box;
    transform-origin: center;
    animation: irisSnap 1.8s cubic-bezier(0.34, 1.56, 0.64, 1) infinite;
  }
  .iris-bl {
    --dx: -3px;
    --dy: 3px;
    --rot: 8deg;
    transform-box: fill-box;
    transform-origin: center;
    animation: irisSnap 1.8s cubic-bezier(0.34, 1.56, 0.64, 1) infinite;
  }

  /* --- 4. Radar Echo --- */
  .echo-ring {
    position: absolute;
    top: 0;
    left: 0;
    animation: echoWave 2s cubic-bezier(0.16, 1, 0.3, 1) infinite;
    pointer-events: none;
  }
  .echo-ring-delayed {
    animation-delay: 0.75s;
  }

  @keyframes echoWave {
    0% {
      transform: scale(1);
      opacity: 0.7;
    }
    100% {
      transform: scale(1.65);
      opacity: 0;
    }
  }

  /* --- 5. Stepped Orbit --- */
  .orbit-svg {
    transform-origin: 20px 20px;
    animation: steppedOrbit 3.2s cubic-bezier(0.65, 0, 0.35, 1) infinite;
  }

  @keyframes steppedOrbit {
    0%,
    15% {
      transform: rotate(0deg);
    }
    25%,
    40% {
      transform: rotate(90deg);
    }
    50%,
    65% {
      transform: rotate(180deg);
    }
    75%,
    90% {
      transform: rotate(270deg);
    }
    100% {
      transform: rotate(360deg);
    }
  }
</style>
