<script>
  import { onMount, onDestroy } from "svelte";

  let { value = "", onChange } = $props();

  let container;
  let editor = null;
  let monaco = null;
  let ignoreChange = false;

  onMount(async () => {
    // Dynamic import to handle Monaco's large bundle
    const mod = await import("monaco-editor");
    monaco = mod;

    // Configure Monaco environment for workers
    self.MonacoEnvironment = {
      getWorker() {
        return new Worker(
          new URL("monaco-editor/esm/vs/editor/editor.worker.js", import.meta.url),
          { type: "module" },
        );
      },
    };

    editor = monaco.editor.create(container, {
      value: value,
      language: "plaintext",
      theme: "vs-dark",
      minimap: { enabled: false },
      lineNumbers: "on",
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      scrollBeyondLastLine: false,
      wordWrap: "on",
      automaticLayout: true,
      padding: { top: 8 },
      renderLineHighlight: "line",
      overviewRulerLanes: 0,
      hideCursorInOverviewRuler: true,
      scrollbar: {
        verticalScrollbarSize: 8,
        horizontalScrollbarSize: 8,
      },
    });

    editor.onDidChangeModelContent(() => {
      if (ignoreChange) return;
      onChange?.(editor.getValue());
    });
  });

  onDestroy(() => {
    editor?.dispose();
  });

  // Update editor when value changes externally
  $effect(() => {
    if (editor && value !== editor.getValue()) {
      ignoreChange = true;
      editor.setValue(value);
      ignoreChange = false;
    }
  });
</script>

<div class="editor-container" bind:this={container}></div>

<style>
  .editor-container {
    flex: 1;
    min-height: 0;
  }
</style>
