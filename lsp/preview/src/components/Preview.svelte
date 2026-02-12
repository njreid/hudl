<script>
  let { html = "", onAction = null } = $props();

  let srcdoc = $derived(
    `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      padding: 16px;
      margin: 0;
      color: #1e1e2e;
      background: #fff;
    }
  </style>
</head>
<body>${html}
<script>
(function() {
  // Datastar action stubs — log calls to parent via postMessage
  var actions = ['get','post','put','patch','delete'];
  actions.forEach(function(method) {
    window['$$' + method] = function(url, opts) {
      window.parent.postMessage({
        type: 'hudl-action',
        action: '@' + method,
        url: url || '',
        opts: opts || null,
        time: Date.now()
      }, '*');
    };
  });

  // Signal actions
  ['setAll','toggleAll','fit','peek','clipboard'].forEach(function(name) {
    window['$$' + name] = function() {
      window.parent.postMessage({
        type: 'hudl-action',
        action: '@' + name,
        args: Array.from(arguments),
        time: Date.now()
      }, '*');
    };
  });

  // Attach event handlers for data-on-* attributes
  function attachHandlers() {
    document.querySelectorAll('*').forEach(function(el) {
      Array.from(el.attributes).forEach(function(attr) {
        var m = attr.name.match(/^data-on-([\\w]+)/);
        if (!m) return;
        var eventName = m[1];
        var expr = attr.value;
        // Skip if already attached
        if (el.dataset['_hudl_bound_' + eventName]) return;
        el.dataset['_hudl_bound_' + eventName] = '1';

        el.addEventListener(eventName, function(e) {
          // Check for action pattern @verb(...)
          var actionMatch = expr.match(/@(\\w+)\\((.*)\\)/);
          if (actionMatch) {
            e.preventDefault();
            window.parent.postMessage({
              type: 'hudl-action',
              action: '@' + actionMatch[1],
              expr: expr,
              event: eventName,
              time: Date.now()
            }, '*');
          } else {
            // Non-action expression — still log it
            window.parent.postMessage({
              type: 'hudl-action',
              action: 'expression',
              expr: expr,
              event: eventName,
              time: Date.now()
            }, '*');
          }
        });
      });
    });
  }

  // Run on load and observe for dynamic changes
  attachHandlers();
  new MutationObserver(attachHandlers).observe(document.body, { childList: true, subtree: true });
})();
<` + `/script>
</body>
</html>`,
  );

  import { onMount, onDestroy } from "svelte";

  function handleMessage(event) {
    if (event.data && event.data.type === "hudl-action") {
      onAction?.(event.data);
    }
  }

  onMount(() => {
    window.addEventListener("message", handleMessage);
  });

  onDestroy(() => {
    window.removeEventListener("message", handleMessage);
  });
</script>

<iframe
  title="Component Preview"
  {srcdoc}
  sandbox="allow-scripts"
></iframe>

<style>
  iframe {
    flex: 1;
    border: none;
    background: white;
    min-height: 0;
  }
</style>
