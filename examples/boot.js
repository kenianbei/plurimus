// Pre-flight and failure display shared by the demo pages. A demo's package
// is fetched only once the browser has shown it can run it.

const BACK = "../";

export async function boot(start, { webgpu = false } = {}) {
  const problem = missingWebGl2() ?? (webgpu ? await webGpuProblem() : null);
  if (problem) {
    notify(problem);
    return;
  }
  addEventListener("error", (event) => stopped(event.error ?? event.message));
  addEventListener("unhandledrejection", (event) => stopped(event.reason));
  document.addEventListener("plurimus-exit", () => location.assign(BACK));
  try {
    await start();
  } catch (error) {
    stopped(error);
  }
}

function missingWebGl2() {
  if (document.createElement("canvas").getContext("webgl2")) {
    return null;
  }
  return ["This browser can't run the demos: it has no WebGL2."];
}

async function webGpuProblem() {
  const adapter = await navigator.gpu?.requestAdapter().catch(() => null);
  if (!adapter) {
    return [
      "The lander renders with WebGPU, which this browser doesn't offer.",
      "Recent Chrome, Edge and Safari support it.",
    ];
  }
  if (adapter.info?.isFallbackAdapter) {
    return [
      "WebGPU is running in software here, which is too slow for the lander.",
      "On Linux, Chrome uses the GPU when started with:",
      "--enable-unsafe-webgpu --enable-features=Vulkan",
    ];
  }
  return null;
}

function stopped(reason) {
  notify([`The demo stopped: ${reason?.message ?? reason}`]);
}

function notify(lines) {
  const notice = document.createElement("div");
  notice.style.cssText =
    "max-width: 40rem; margin: 4rem auto; padding: 0 1rem; " +
    "color: #ddd; font: 1rem/1.5 system-ui, sans-serif;";
  for (const line of lines) {
    const paragraph = document.createElement("p");
    paragraph.textContent = line;
    notice.append(paragraph);
  }
  const back = document.createElement("a");
  back.href = BACK;
  back.textContent = "Back to the demos";
  back.style.color = "#8cf";
  notice.append(back);
  document.body.replaceChildren(notice);
}
