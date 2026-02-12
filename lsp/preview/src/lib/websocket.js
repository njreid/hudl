/**
 * WebSocket connection with auto-reconnect.
 * Calls `onMessage(data)` for each parsed JSON message.
 * Calls `onStatusChange(connected)` on connect/disconnect.
 */
export function createWebSocket({ onMessage, onStatusChange }) {
  let ws = null;
  let reconnectTimer = null;

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${location.host}/ws`;

    ws = new WebSocket(url);

    ws.onopen = () => {
      onStatusChange?.(true);
    };

    ws.onclose = () => {
      onStatusChange?.(false);
      scheduleReconnect();
    };

    ws.onerror = () => {
      ws?.close();
    };

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        onMessage?.(data);
      } catch {
        // ignore non-JSON messages
      }
    };
  }

  function scheduleReconnect() {
    if (reconnectTimer) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, 2000);
  }

  function disconnect() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
    ws?.close();
    ws = null;
  }

  connect();

  return { disconnect };
}
