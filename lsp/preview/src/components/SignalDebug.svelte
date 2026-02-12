<script>
  let { html = "", visible = true } = $props();

  let signals = $derived.by(() => {
    if (!html) return [];

    const entries = new Map();

    // Match data-signals-{name}="value"
    const sigRe = /data-signals-(\w+)="([^"]*)"/g;
    let m;
    while ((m = sigRe.exec(html)) !== null) {
      entries.set(m[1], { name: "$" + m[1], value: m[2], kind: "signal" });
    }

    // Match data-computed-{name}="value"
    const compRe = /data-computed-(\w+)="([^"]*)"/g;
    while ((m = compRe.exec(html)) !== null) {
      entries.set(m[1], { name: "~" + m[1], value: m[2], kind: "computed" });
    }

    return Array.from(entries.values());
  });
</script>

{#if visible}
  <div class="signal-debug">
    <div class="panel-header">Signals</div>
    <div class="signal-list">
      {#if signals.length === 0}
        <div class="empty">No signals found</div>
      {:else}
        {#each signals as sig}
          <div class="signal-row">
            <span class="signal-name" class:computed={sig.kind === "computed"}>{sig.name}</span>
            <span class="signal-value">{sig.value}</span>
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .signal-debug {
    width: 220px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
  }

  .panel-header {
    display: flex;
    align-items: center;
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

  .signal-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .empty {
    color: var(--text-dim);
    font-size: 12px;
    padding: 8px;
  }

  .signal-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
  }

  .signal-row:hover {
    background: var(--bg-hover);
  }

  .signal-name {
    color: var(--accent);
    font-family: monospace;
    flex-shrink: 0;
  }

  .signal-name.computed {
    color: var(--accent-hover);
  }

  .signal-value {
    color: var(--text-dim);
    font-family: monospace;
    margin-left: 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
