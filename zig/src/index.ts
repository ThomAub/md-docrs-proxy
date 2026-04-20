import mdDocrsWasm from "./md_docrs.wasm";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface WasmExports {
  memory: WebAssembly.Memory;
  alloc: (len: number) => number;
  free: (ptr: number, len: number) => void;
  resolve_url: (
    specPtr: number,
    specLen: number,
    targetPtr: number,
    targetLen: number,
    outPtr: number,
    outCap: number,
  ) => number;
}

const OUT_CAP = 512;

// Wrangler compiles the imported .wasm to a WebAssembly.Module at build time.
// Each request instantiates a fresh module: the Zig wasm_allocator has no
// global state we need to reset, but allocator arenas and Zig globals are
// cheap to recreate and it keeps requests isolated.
function resolveUrl(spec: string, target: string | null): string {
  const instance = new WebAssembly.Instance(mdDocrsWasm);
  const wasm = instance.exports as unknown as WasmExports;

  const specBytes = encoder.encode(spec);
  const targetBytes = target ? encoder.encode(target) : new Uint8Array(0);

  const specPtr = wasm.alloc(specBytes.length);
  const targetPtr = targetBytes.length ? wasm.alloc(targetBytes.length) : 0;
  const outPtr = wasm.alloc(OUT_CAP);
  if (specPtr === 0 || outPtr === 0 || (targetBytes.length && targetPtr === 0)) {
    throw new Error("WASM alloc failed");
  }

  try {
    // Views must be created *after* every alloc: WASM memory growth detaches
    // any existing Uint8Array onto the old buffer. See ../doc-memory notes.
    new Uint8Array(wasm.memory.buffer, specPtr, specBytes.length).set(specBytes);
    if (targetBytes.length) {
      new Uint8Array(wasm.memory.buffer, targetPtr, targetBytes.length).set(targetBytes);
    }

    const n = wasm.resolve_url(
      specPtr,
      specBytes.length,
      targetPtr,
      targetBytes.length,
      outPtr,
      OUT_CAP,
    );
    if (n === 0) throw new Error("invalid spec or output buffer too small");

    return decoder.decode(new Uint8Array(wasm.memory.buffer, outPtr, n));
  } finally {
    wasm.free(specPtr, specBytes.length);
    if (targetBytes.length) wasm.free(targetPtr, targetBytes.length);
    wasm.free(outPtr, OUT_CAP);
  }
}

export default {
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    // Spec can come from ?spec=... or the path (/serde::de::Deserialize).
    const spec = url.searchParams.get("spec") ?? decodeURIComponent(url.pathname.replace(/^\//, ""));
    if (!spec) {
      return new Response(
        "usage: GET /<spec>[?target=<triple>]\n" +
          "example: /tokio@1.52.1::sync::Mutex?target=x86_64-unknown-linux-gnu\n",
        { status: 400, headers: { "Content-Type": "text/plain; charset=utf-8" } },
      );
    }

    const target = url.searchParams.get("target");
    try {
      const docsUrl = resolveUrl(spec, target);
      return new Response(docsUrl + "\n", {
        headers: { "Content-Type": "text/plain; charset=utf-8" },
      });
    } catch (err) {
      return new Response(`error: ${(err as Error).message}\n`, {
        status: 400,
        headers: { "Content-Type": "text/plain; charset=utf-8" },
      });
    }
  },
};
