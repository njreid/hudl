<script>
  import { onMount, onDestroy } from "svelte";
  import ComponentSelect from "./components/ComponentSelect.svelte";
  import Editor from "./components/Editor.svelte";
  import Preview from "./components/Preview.svelte";
  import SignalDebug from "./components/SignalDebug.svelte";
  import ActionLog from "./components/ActionLog.svelte";
  import { fetchComponents, renderPreview, loadPreviewData, savePreviewData, fetchPreviewFiles, createPreviewFile } from "./lib/api.js";
  import { createWebSocket } from "./lib/websocket.js";

  let components = $state([]);
  let selectedComponent = $state(null);
  let previewFiles = $state([]);
  let selectedFile = $state("default");
  let textproto = $state("");
  let html = $state("");
  let error = $state(null);
  let renderTime = $state(null);
  let connected = $state(false);
  let saving = $state(false);
  let showSignals = $state(true);
  let showActions = $state(true);
  let actionLog = $state([]);

  let renderTimer = null;
  let saveTimer = null;
  let ws = null;

  async function loadComponents() {
    try {
      components = await fetchComponents();
    } catch (e) {
      error = "Failed to load components: " + e.message;
    }
  }

  async function selectComponent(name) {
    selectedComponent = name;
    selectedFile = "default";
    previewFiles = [];
    error = null;
    html = "";

    if (!name) return;

    try {
      // Load preview files list and default data in parallel
      const [files, data] = await Promise.all([
        fetchPreviewFiles(name).catch(() => []),
        loadPreviewData(name, "default"),
      ]);
      previewFiles = files;
      textproto = data.textproto || "";
      await doRender();
      if (data.source === "skeleton" || data.source === "created") {
        savePreviewData(name, textproto, "default").catch(() => {});
        // Refresh files list since a default file may have been created
        previewFiles = await fetchPreviewFiles(name).catch(() => []);
      }
    } catch (e) {
      error = "Failed to load preview data: " + e.message;
    }
  }

  async function selectFile(file) {
    selectedFile = file;
    error = null;

    if (!selectedComponent) return;

    try {
      const data = await loadPreviewData(selectedComponent, file);
      textproto = data.textproto || "";
      await doRender();
    } catch (e) {
      error = "Failed to load preview data: " + e.message;
    }
  }

  async function addPreviewFile() {
    if (!selectedComponent) return;
    const label = prompt("Preview file name (e.g. 'empty', 'error-state'):");
    if (!label) return;

    try {
      const result = await createPreviewFile(selectedComponent, label);
      if (result.error) {
        error = result.error;
        return;
      }
      // Refresh the files list and select the new file
      previewFiles = await fetchPreviewFiles(selectedComponent).catch(() => []);
      selectedFile = label;
      textproto = result.textproto || "";
      await doRender();
    } catch (e) {
      error = "Failed to create preview file: " + e.message;
    }
  }

  async function doRender() {
    if (!selectedComponent) return;

    try {
      const result = await renderPreview(selectedComponent, textproto);
      if (result.error) {
        error = result.error;
        html = "";
      } else {
        error = null;
        html = result.html;
        renderTime = result.render_time_ms;
      }
    } catch (e) {
      error = "Render failed: " + e.message;
    }
  }

  function onEditorChange(value) {
    textproto = value;

    // Debounced render (300ms)
    clearTimeout(renderTimer);
    renderTimer = setTimeout(() => doRender(), 300);

    // Debounced save (2s)
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => doSave(), 2000);
  }

  async function doSave() {
    if (!selectedComponent) return;
    saving = true;
    try {
      await savePreviewData(selectedComponent, textproto, selectedFile);
    } catch {
      // silent save failure
    }
    saving = false;
  }

  function onAction(data) {
    actionLog = [...actionLog, data].slice(-50); // keep last 50 entries
  }

  function clearActions() {
    actionLog = [];
  }

  onMount(() => {
    loadComponents();

    ws = createWebSocket({
      onMessage(data) {
        if (data.type === "reload") {
          // Refresh component list and re-render current
          loadComponents();
          if (selectedComponent && data.component === selectedComponent) {
            doRender();
          }
        }
      },
      onStatusChange(isConnected) {
        connected = isConnected;
      },
    });
  });

  onDestroy(() => {
    clearTimeout(renderTimer);
    clearTimeout(saveTimer);
    ws?.disconnect();
  });
</script>

<div class="layout">
  <header>
    <div class="header-left">
      <span class="logo">Hudl Preview</span>
      <ComponentSelect {components} selected={selectedComponent} onSelect={selectComponent} />
      {#if selectedComponent && previewFiles.length > 0}
        <div class="file-select">
          <select value={selectedFile} onchange={(e) => selectFile(e.target.value)}>
            {#each previewFiles as pf}
              <option value={pf.file}>{pf.label}</option>
            {/each}
          </select>
          <button class="add-file-btn" onclick={addPreviewFile} title="New preview file">+</button>
        </div>
      {/if}
    </div>
    <div class="header-right">
      {#if saving}
        <span class="status saving">Saving...</span>
      {/if}
      {#if renderTime}
        <span class="status render-time">{renderTime}ms</span>
      {/if}
      <span class="status" class:connected class:disconnected={!connected}>
        {connected ? "Connected" : "Disconnected"}
      </span>
      <button class="toggle-signals" class:active={showSignals} onclick={() => showSignals = !showSignals}>
        Signals
      </button>
      <button class="toggle-signals" class:active={showActions} onclick={() => showActions = !showActions}>
        Actions
      </button>
    </div>
  </header>

  <main>
    {#if selectedComponent}
      <div class="editor-panel">
        <div class="panel-header">TextProto Data</div>
        <Editor value={textproto} onChange={onEditorChange} />
      </div>
      <div class="preview-panel">
        <div class="panel-header">
          Preview
          {#if error}
            <span class="panel-error">{error}</span>
          {/if}
        </div>
        <Preview {html} {onAction} />
      </div>
      <SignalDebug {html} visible={showSignals} />
      <ActionLog actions={actionLog} visible={showActions} onClear={clearActions} />
    {:else}
      <div class="empty-state">
        <p>Select a component to preview</p>
      </div>
    {/if}
  </main>
</div>

<style>
  .layout {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--header-height);
    padding: 0 16px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo {
    font-weight: 600;
    color: var(--accent);
    font-size: 15px;
  }

  .file-select {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .file-select select {
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 12px;
  }

  .file-select select:focus {
    outline: none;
    border-color: var(--accent);
  }

  .add-file-btn {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    font-weight: 600;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    background: transparent;
    cursor: pointer;
  }

  .add-file-btn:hover {
    background: var(--bg-hover);
    color: var(--accent);
    border-color: var(--accent);
  }

  .status {
    font-size: 12px;
    color: var(--text-dim);
  }

  .status.connected {
    color: var(--success);
  }

  .status.disconnected {
    color: var(--error);
  }

  .status.saving {
    color: var(--accent);
  }

  .render-time {
    font-variant-numeric: tabular-nums;
  }

  main {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .editor-panel,
  .preview-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .editor-panel {
    border-right: 1px solid var(--border);
  }

  .panel-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
    background: var(--bg-surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .panel-error {
    color: var(--error);
    font-weight: 400;
    text-transform: none;
    letter-spacing: normal;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .toggle-signals {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    border: 1px solid var(--border);
    color: var(--text-dim);
    background: transparent;
  }

  .toggle-signals.active {
    color: var(--accent);
    border-color: var(--accent);
  }

  .toggle-signals:hover {
    background: var(--bg-hover);
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }
</style>
