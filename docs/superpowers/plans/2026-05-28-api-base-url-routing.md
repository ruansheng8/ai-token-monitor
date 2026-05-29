# API Base URL Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure frontend API requests always hit the local Axum backend instead of receiving `index.html` and failing JSON parsing.

**Architecture:** Add a small frontend API helper that builds URLs against `http://127.0.0.1:19362/api` when running inside Tauri and keeps relative `/api` URLs for normal browser development. Route every `App.tsx` API request through this helper and parse JSON through a guard that reports HTML fallback responses clearly.

**Tech Stack:** React 19, TypeScript, Vite, Tauri v2, Axum backend on port 19362.

---

## File Structure

- Create `src/lib/api.ts`: owns API base URL detection, API URL construction, and JSON response parsing.
- Modify `src/App.tsx`: replace direct `/api/...` fetch URLs with `apiUrl(...)`; replace direct `response.json()` for app APIs with `readJsonResponse(...)`.
- Verify with `npm run build` and direct API probing against `http://127.0.0.1:19362/api/config`.

---

### Task 1: API Helper

**Files:**
- Create: `src/lib/api.ts`

- [ ] **Step 1: Write a failing helper smoke check**

Run this PowerShell command before creating `src/lib/api.ts`:

```powershell
if (Test-Path "src/lib/api.ts") { Write-Output "PASS: helper exists" } else { Write-Output "FAIL: helper missing"; exit 1 }
```

Expected: FAIL with `helper missing`.

- [ ] **Step 2: Create the API helper**

Create `src/lib/api.ts` with:

```ts
const API_PORT = 19362;

const isTauriRuntime = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const apiUrl = (path: string) => {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  const apiPath = normalizedPath.startsWith('/api/') || normalizedPath === '/api'
    ? normalizedPath
    : `/api${normalizedPath}`;

  if (isTauriRuntime()) {
    return `http://127.0.0.1:${API_PORT}${apiPath}`;
  }

  return apiPath;
};

export const readJsonResponse = async <T>(response: Response): Promise<T> => {
  const text = await response.text();
  const trimmed = text.trimStart();

  if (trimmed.startsWith('<!doctype') || trimmed.startsWith('<html')) {
    throw new Error('API 返回了 HTML 页面，说明请求没有命中本地后台服务。');
  }

  return JSON.parse(text) as T;
};
```

- [ ] **Step 3: Verify helper exists**

Run:

```powershell
if (Test-Path "src/lib/api.ts") { Write-Output "PASS: helper exists" } else { Write-Output "FAIL: helper missing"; exit 1 }
```

Expected: PASS.

---

### Task 2: Route App Fetches Through Helper

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Write failing direct-fetch check**

Run:

```powershell
$matches = Select-String -Path "src/App.tsx" -Pattern "fetch\(([''\"]|`)\/api" -AllMatches
if ($matches) { Write-Output "FAIL: direct /api fetch remains"; exit 1 } else { Write-Output "PASS: no direct /api fetch" }
```

Expected: FAIL with `direct /api fetch remains`.

- [ ] **Step 2: Import API helpers**

Add this import after the Tauri event import in `src/App.tsx`:

```ts
import { apiUrl, readJsonResponse } from './lib/api';
```

- [ ] **Step 3: Replace API fetch URLs**

Replace each app API fetch URL as follows:

```ts
fetch('/api/db/clean', { method: 'POST' })
```

becomes:

```ts
fetch(apiUrl('/db/clean'), { method: 'POST' })
```

```ts
fetch(`/api/config?t=${Date.now()}`)
```

becomes:

```ts
fetch(apiUrl(`/config?t=${Date.now()}`))
```

```ts
fetch(`/api/scan/status?t=${Date.now()}`)
```

becomes:

```ts
fetch(apiUrl(`/scan/status?t=${Date.now()}`))
```

```ts
fetch(`/api/scan/start?t=${Date.now()}`)
```

becomes:

```ts
fetch(apiUrl(`/scan/start?t=${Date.now()}`))
```

```ts
fetch(`/api/metrics?source=${currentSource}&start_date=${start}&end_date=${end}&t=${Date.now()}`, { signal: controller.signal })
```

becomes:

```ts
fetch(apiUrl(`/metrics?source=${currentSource}&start_date=${start}&end_date=${end}&t=${Date.now()}`), { signal: controller.signal })
```

```ts
fetch(`/api/sessions?${query.toString()}`)
```

becomes:

```ts
fetch(apiUrl(`/sessions?${query.toString()}`))
```

```ts
fetch('/api/config/test', { ... })
```

becomes:

```ts
fetch(apiUrl('/config/test'), { ... })
```

```ts
fetch('/api/config/save', { ... })
```

becomes:

```ts
fetch(apiUrl('/config/save'), { ... })
```

```ts
fetch('/api/app/restart', { method: 'POST' })
```

becomes:

```ts
fetch(apiUrl('/app/restart'), { method: 'POST' })
```

- [ ] **Step 4: Replace JSON parsing for app APIs**

Replace direct app API JSON parsing calls:

```ts
await response.json()
await res.json()
await configRes.json()
await scanRes.json()
await metricsRes.json()
await sessionsRes.json()
```

with typed `readJsonResponse(...)` calls at each site. Use existing local variable names and types where present, for example:

```ts
const result: AggregatedMetrics = await readJsonResponse(response);
const configData = await readJsonResponse<any>(configRes);
const scanStatusVal = await readJsonResponse<typeof scanStatus>(scanRes);
const metricsVal = await readJsonResponse<AggregatedMetrics>(metricsRes);
const sessionsVal = await readJsonResponse<{ items: SessionItem[]; total: number }>(sessionsRes);
```

- [ ] **Step 5: Verify direct /api fetches are gone**

Run:

```powershell
$matches = Select-String -Path "src/App.tsx" -Pattern "fetch\(([''\"]|`)\/api" -AllMatches
if ($matches) { $matches | ForEach-Object { $_.Line }; exit 1 } else { Write-Output "PASS: no direct /api fetch" }
```

Expected: PASS.

---

### Task 3: Verification

**Files:**
- No new code expected beyond Tasks 1-2.

- [ ] **Step 1: Build**

Run:

```powershell
npm run build
```

Expected: TypeScript and Vite build complete with exit 0.

- [ ] **Step 2: Probe backend JSON directly**

Run:

```powershell
$r = Invoke-WebRequest -Uri "http://127.0.0.1:19362/api/config?t=$(Get-Date -UFormat %s)" -UseBasicParsing -TimeoutSec 5
Write-Output "status=$($r.StatusCode) contentType=$($r.Headers['Content-Type']) first=$($r.Content.Substring(0, [Math]::Min(40, $r.Content.Length)))"
```

Expected: status 200, content type `application/json; charset=utf-8`, and response starting with `{`.

- [ ] **Step 3: Final direct-fetch check**

Run:

```powershell
$matches = Select-String -Path "src/App.tsx" -Pattern "fetch\(([''\"]|`)\/api" -AllMatches
if ($matches) { $matches | ForEach-Object { $_.Line }; exit 1 } else { Write-Output "PASS: no direct /api fetch" }
```

Expected: PASS.

---

## Self-Review

- Spec coverage: covers absolute Tauri API base URL, unified fetch routing, clearer HTML-as-JSON errors, and build/backend verification.
- Placeholder scan: no TBD/TODO/placeholders remain.
- Type consistency: `apiUrl` and `readJsonResponse` names are used consistently in `App.tsx` tasks.
