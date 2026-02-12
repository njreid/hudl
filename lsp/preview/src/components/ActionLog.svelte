<script>
  let { actions = [], visible = true, onClear = null } = $props();
</script>

{#if visible}
  <div class="action-log">
    <div class="panel-header">
      Actions
      {#if actions.length > 0}
        <button class="clear-btn" onclick={onClear}>Clear</button>
      {/if}
    </div>
    <div class="log-list">
      {#if actions.length === 0}
        <div class="empty">No actions logged yet. Click elements with @action handlers to see them here.</div>
      {:else}
        {#each actions as entry, i}
          <div class="log-entry">
            <span class="log-action" class:http={entry.action.match(/^@(get|post|put|patch|delete)$/)}>{entry.action}</span>
            <span class="log-detail">{entry.expr || entry.url || entry.args?.join(", ") || ""}</span>
            {#if entry.event}
              <span class="log-event">on:{entry.event}</span>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .action-log {
    width: 280px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
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

  .clear-btn {
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-dim);
    background: transparent;
    cursor: pointer;
    text-transform: none;
    letter-spacing: normal;
    font-weight: 400;
  }

  .clear-btn:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .log-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }

  .empty {
    color: var(--text-dim);
    font-size: 12px;
    padding: 8px;
    line-height: 1.4;
  }

  .log-entry {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
    border-bottom: 1px solid var(--border);
  }

  .log-entry:last-child {
    border-bottom: none;
  }

  .log-entry:hover {
    background: var(--bg-hover);
  }

  .log-action {
    color: var(--accent);
    font-family: monospace;
    font-weight: 600;
    flex-shrink: 0;
  }

  .log-action.http {
    color: #f5a623;
  }

  .log-detail {
    color: var(--text);
    font-family: monospace;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .log-event {
    color: var(--text-dim);
    font-size: 10px;
    font-family: monospace;
  }
</style>
