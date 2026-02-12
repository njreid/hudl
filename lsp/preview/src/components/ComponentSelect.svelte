<script>
  let { components = [], selected = null, onSelect } = $props();
</script>

<select
  value={selected || ""}
  onchange={(e) => onSelect(e.target.value || null)}
>
  <option value="">-- Select component --</option>
  {#each components as comp}
    <option value={comp.name}>
      {comp.name}
      {#if comp.data_type}
        ({comp.data_type})
      {/if}
    </option>
  {/each}
</select>

<style>
  select {
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 13px;
    min-width: 200px;
  }

  select:focus {
    outline: none;
    border-color: var(--accent);
  }
</style>
