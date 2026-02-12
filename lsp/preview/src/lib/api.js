const BASE = "";

export async function fetchComponents() {
  const res = await fetch(`${BASE}/api/components`);
  if (!res.ok) throw new Error(`Failed to fetch components: ${res.status}`);
  return res.json();
}

export async function fetchProtoSchema(name) {
  const res = await fetch(`${BASE}/api/proto-schema/${encodeURIComponent(name)}`);
  if (!res.ok) throw new Error(`Failed to fetch schema: ${res.status}`);
  return res.json();
}

export async function renderPreview(component, textproto) {
  const res = await fetch(`${BASE}/api/render-preview`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ component, textproto }),
  });
  return res.json();
}

export async function loadPreviewData(name, file = "default") {
  const params = file && file !== "default" ? `?file=${encodeURIComponent(file)}` : "";
  const res = await fetch(`${BASE}/api/preview-data/${encodeURIComponent(name)}${params}`);
  if (!res.ok) throw new Error(`Failed to load preview data: ${res.status}`);
  return res.json();
}

export async function savePreviewData(name, textproto, file = "default") {
  const params = file && file !== "default" ? `?file=${encodeURIComponent(file)}` : "";
  const res = await fetch(`${BASE}/api/preview-data/${encodeURIComponent(name)}${params}`, {
    method: "PUT",
    headers: { "Content-Type": "text/plain" },
    body: textproto,
  });
  return res.json();
}

export async function fetchPreviewFiles(name) {
  const res = await fetch(`${BASE}/api/preview-files/${encodeURIComponent(name)}`);
  if (!res.ok) throw new Error(`Failed to fetch preview files: ${res.status}`);
  return res.json();
}

export async function createPreviewFile(name, label) {
  const res = await fetch(`${BASE}/api/preview-files/${encodeURIComponent(name)}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ label }),
  });
  return res.json();
}
