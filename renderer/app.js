const state = {
  categories: [],
  scanResults: new Map(),
  selectedIds: new Set(),
  includeAdvanced: false,
  deleteMode: "permanent",
  showOnlyDetected: false,
  searchQuery: "",
  sortMode: "space-desc",
  isScanning: false,
  isCleaning: false,
  lastScanResult: null,
  systemOverview: null,
};

const elements = {
  totalBytes: document.getElementById("totalBytes"),
  totalFiles: document.getElementById("totalFiles"),
  selectedBytes: document.getElementById("selectedBytes"),
  statusText: document.getElementById("statusText"),
  categoryCount: document.getElementById("categoryCount"),
  categoryContainer: document.getElementById("categoryContainer"),
  logOutput: document.getElementById("logOutput"),
  topWasteList: document.getElementById("topWasteList"),
  systemDriveFree: document.getElementById("systemDriveFree"),
  cpuInfo: document.getElementById("cpuInfo"),
  memoryInfo: document.getElementById("memoryInfo"),
  searchInput: document.getElementById("searchInput"),
  sortSelect: document.getElementById("sortSelect"),
  scanBtn: document.getElementById("scanBtn"),
  cleanBtn: document.getElementById("cleanBtn"),
  exportBtn: document.getElementById("exportBtn"),
  advancedToggle: document.getElementById("advancedToggle"),
  recycleModeToggle: document.getElementById("recycleModeToggle"),
  onlyDetectedToggle: document.getElementById("onlyDetectedToggle"),
  presetRecommendedBtn: document.getElementById("presetRecommendedBtn"),
  presetDeepBtn: document.getElementById("presetDeepBtn"),
  presetAggressiveBtn: document.getElementById("presetAggressiveBtn"),
  selectTopSpaceBtn: document.getElementById("selectTopSpaceBtn"),
  selectRecommendedBtn: document.getElementById("selectRecommendedBtn"),
  selectVisibleBtn: document.getElementById("selectVisibleBtn"),
  clearSelectionBtn: document.getElementById("clearSelectionBtn"),
};

function escapeHtml(input) {
  return String(input)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${units[unitIndex]}`;
}

function appendLog(message) {
  const time = new Date().toLocaleTimeString();
  elements.logOutput.textContent = `[${time}] ${message}\n${elements.logOutput.textContent}`;
}

function setStatus(text) {
  elements.statusText.textContent = text;
}

function riskWeight(risk) {
  if (risk === "high") return 3;
  if (risk === "medium") return 2;
  return 1;
}

function bytesForCategory(categoryId) {
  return state.scanResults.get(categoryId)?.bytes || 0;
}

function filesForCategory(categoryId) {
  return state.scanResults.get(categoryId)?.files || 0;
}

function issuesForCategory(categoryId) {
  return state.scanResults.get(categoryId)?.issues || [];
}

function getFilteredCategories() {
  const query = state.searchQuery.trim().toLowerCase();

  const filtered = state.categories.filter((category) => {
    if (!state.includeAdvanced && category.advanced) {
      return false;
    }

    if (state.showOnlyDetected && bytesForCategory(category.id) <= 0) {
      return false;
    }

    if (!query) {
      return true;
    }

    const haystack = [category.title, category.description, category.whyItHelps, ...(category.targets || [])]
      .join(" ")
      .toLowerCase();
    return haystack.includes(query);
  });

  filtered.sort((a, b) => {
    if (state.sortMode === "title-asc") {
      return a.title.localeCompare(b.title);
    }

    if (state.sortMode === "risk-desc") {
      return riskWeight(b.risk) - riskWeight(a.risk);
    }

    if (state.sortMode === "space-asc") {
      return bytesForCategory(a.id) - bytesForCategory(b.id);
    }

    return bytesForCategory(b.id) - bytesForCategory(a.id);
  });

  return filtered;
}

function selectedTotalBytes() {
  let bytes = 0;
  for (const id of state.selectedIds) {
    bytes += bytesForCategory(id);
  }
  return bytes;
}

function updateTopMetrics() {
  let totalBytes = 0;
  let totalFiles = 0;
  for (const result of state.scanResults.values()) {
    totalBytes += result.bytes;
    totalFiles += result.files;
  }

  elements.totalBytes.textContent = formatBytes(totalBytes);
  elements.totalFiles.textContent = totalFiles.toLocaleString();
  elements.selectedBytes.textContent = formatBytes(selectedTotalBytes());
}

function renderTopWaste() {
  const top = [...state.scanResults.entries()]
    .map(([id, result]) => {
      const category = state.categories.find((item) => item.id === id);
      return {
        title: category ? category.title : id,
        bytes: result.bytes,
      };
    })
    .filter((item) => item.bytes > 0)
    .sort((a, b) => b.bytes - a.bytes)
    .slice(0, 5);

  if (top.length === 0) {
    elements.topWasteList.innerHTML = "<li>No scan data yet.</li>";
    return;
  }

  elements.topWasteList.innerHTML = top
    .map((item) => `<li>${escapeHtml(item.title)} - ${formatBytes(item.bytes)}</li>`)
    .join("");
}

function renderSystemOverview() {
  if (!state.systemOverview) {
    elements.systemDriveFree.textContent = "Unavailable";
    elements.cpuInfo.textContent = "Unavailable";
    elements.memoryInfo.textContent = "Unavailable";
    return;
  }

  const drive = state.systemOverview.systemDrive || { free: 0, size: 0 };
  const used = Math.max(0, drive.size - drive.free);

  elements.systemDriveFree.textContent = `${formatBytes(drive.free)} free / ${formatBytes(drive.size)} total`;
  elements.cpuInfo.textContent = `${state.systemOverview.cpuCores} cores`;
  elements.memoryInfo.textContent = `${formatBytes(state.systemOverview.memoryFree)} free / ${formatBytes(
    state.systemOverview.memoryTotal
  )}`;

  appendLog(
    `System overview loaded: drive used ${formatBytes(used)}, free ${formatBytes(
      drive.free
    )}, user ${state.systemOverview.user}.`
  );
}

function renderCategories() {
  const visible = getFilteredCategories();
  elements.categoryCount.textContent = `Cleanup Categories (${visible.length})`;

  if (visible.length === 0) {
    elements.categoryContainer.innerHTML = '<p class="muted-note">No categories match your filters.</p>';
    elements.cleanBtn.disabled = true;
    updateTopMetrics();
    return;
  }

  const fragment = document.createDocumentFragment();

  for (const category of visible) {
    const selected = state.selectedIds.has(category.id);
    const bytes = bytesForCategory(category.id);
    const files = filesForCategory(category.id);
    const issueCount = issuesForCategory(category.id).length;

    const visibleTargets = (category.targets || []).slice(0, 3);
    const remainingTargets = Math.max(0, (category.targets || []).length - visibleTargets.length);

    const card = document.createElement("article");
    card.className = `category-card${selected ? " selected" : ""}`;
    card.innerHTML = `
      <div class="card-top">
        <label class="card-title">
          <input type="checkbox" data-id="${escapeHtml(category.id)}" ${selected ? "checked" : ""} ${
      state.isCleaning || state.isScanning ? "disabled" : ""
    } />
          <span>${escapeHtml(category.title)}</span>
        </label>
        <span class="badge ${escapeHtml(category.risk)}">${escapeHtml(category.risk)}</span>
      </div>
      <p class="card-desc">${escapeHtml(category.description)}</p>
      <p class="card-why">${escapeHtml(category.whyItHelps)}</p>
      <div class="card-metrics">
        <span>${formatBytes(bytes)}</span>
        <span>${files.toLocaleString()} files</span>
      </div>
      <ul class="card-targets">
        ${visibleTargets
          .map(
            (target) =>
              `<li class="target-item"><span class="target-path" title="${escapeHtml(target)}">${escapeHtml(
                target
              )}</span></li>`
          )
          .join("")}
      </ul>
      ${remainingTargets > 0 ? `<p class="muted-note">+${remainingTargets} more targets</p>` : ""}
      ${issueCount > 0 ? `<p class="muted-note">Scan notes: ${issueCount}</p>` : ""}
    `;
    fragment.appendChild(card);
  }

  elements.categoryContainer.innerHTML = "";
  elements.categoryContainer.appendChild(fragment);

  elements.categoryContainer.querySelectorAll('input[type="checkbox"]').forEach((checkbox) => {
    checkbox.addEventListener("change", (event) => {
      const id = event.target.dataset.id;
      if (event.target.checked) {
        state.selectedIds.add(id);
      } else {
        state.selectedIds.delete(id);
      }
      updateTopMetrics();
      renderCategories();
    });
  });

  elements.cleanBtn.disabled = state.selectedIds.size === 0 || state.isScanning || state.isCleaning;
  updateTopMetrics();
}

function clearSelection() {
  state.selectedIds.clear();
  renderCategories();
}

function selectRecommended() {
  state.selectedIds.clear();
  for (const category of getFilteredCategories()) {
    if (category.safeDefault) {
      state.selectedIds.add(category.id);
    }
  }
  renderCategories();
}

function selectDeepClean() {
  state.selectedIds.clear();
  for (const category of getFilteredCategories()) {
    if (category.risk !== "high") {
      state.selectedIds.add(category.id);
    }
  }
  renderCategories();
}

function selectAggressive() {
  state.selectedIds.clear();
  for (const category of getFilteredCategories()) {
    state.selectedIds.add(category.id);
  }
  renderCategories();
}

function selectVisible() {
  for (const category of getFilteredCategories()) {
    state.selectedIds.add(category.id);
  }
  renderCategories();
}

function selectTopSpaceHogs() {
  const top = getFilteredCategories()
    .sort((a, b) => bytesForCategory(b.id) - bytesForCategory(a.id))
    .slice(0, 5);

  state.selectedIds.clear();
  for (const category of top) {
    if (bytesForCategory(category.id) > 0) {
      state.selectedIds.add(category.id);
    }
  }
  renderCategories();
}

async function loadCategories() {
  state.categories = await window.cleanerApi.getCategories();
  selectRecommended();
  appendLog(`Loaded ${state.categories.length} cleanup categories.`);
}

async function loadSystemOverview() {
  try {
    state.systemOverview = await window.cleanerApi.getSystemOverview();
    renderSystemOverview();
  } catch (error) {
    appendLog(`System overview unavailable: ${error.message}`);
  }
}

async function runScan() {
  if (state.isScanning || state.isCleaning) {
    return;
  }

  state.isScanning = true;
  elements.scanBtn.disabled = true;
  elements.cleanBtn.disabled = true;
  setStatus("Scanning your system...");
  appendLog("Scan started.");

  try {
    const result = await window.cleanerApi.scan({ includeAdvanced: state.includeAdvanced });
    state.lastScanResult = result;
    state.scanResults = new Map(result.results.map((entry) => [entry.id, entry]));

    renderCategories();
    renderTopWaste();
    appendLog(
      `Scan complete. Found ${formatBytes(result.totalBytes)} in ${result.totalFiles.toLocaleString()} files.`
    );
    setStatus("Scan complete");
  } catch (error) {
    appendLog(`Scan failed: ${error.message}`);
    setStatus("Scan failed");
  } finally {
    state.isScanning = false;
    elements.scanBtn.disabled = false;
    elements.cleanBtn.disabled = state.selectedIds.size === 0;
    updateTopMetrics();
  }
}

function getRiskySelection() {
  const risks = [];
  for (const category of state.categories) {
    if (state.selectedIds.has(category.id) && category.risk === "high") {
      risks.push(category.title);
    }
  }
  return risks;
}

async function runClean() {
  if (state.isScanning || state.isCleaning || state.selectedIds.size === 0) {
    return;
  }

  const selectedIds = [...state.selectedIds];
  const risky = getRiskySelection();

  if (risky.length > 0) {
    const confirmRisk = window.confirm(
      `You selected advanced actions:\n- ${risky.join("\n- ")}\n\nContinue?`
    );
    if (!confirmRisk) return;
  }

  const confirmCleanup = window.confirm(
    `Clean ${selectedIds.length} selected categories now?\n\nDelete mode: ${state.deleteMode === "recycle" ? "Move to Recycle Bin" : "Permanent delete"}.`
  );
  if (!confirmCleanup) return;

  state.isCleaning = true;
  elements.scanBtn.disabled = true;
  elements.cleanBtn.disabled = true;
  setStatus("Cleaning in progress...");
  appendLog(`Cleaning started for ${selectedIds.length} categories. Mode: ${state.deleteMode}.`);

  try {
    const result = await window.cleanerApi.clean({
      ids: selectedIds,
      deleteMode: state.deleteMode,
    });

    appendLog(`Clean complete. Reclaimed ${formatBytes(result.totalReclaimedBytes)}.`);
    for (const item of result.results) {
      appendLog(
        `${item.title}: reclaimed ${formatBytes(item.reclaimedBytes)}${
          item.issues.length ? `, issues ${item.issues.length}` : ""
        }`
      );
    }

    setStatus("Clean complete");
    await runScan();
  } catch (error) {
    appendLog(`Cleanup failed: ${error.message}`);
    setStatus("Clean failed");
  } finally {
    state.isCleaning = false;
    elements.scanBtn.disabled = false;
    elements.cleanBtn.disabled = state.selectedIds.size === 0;
    renderCategories();
  }
}

async function exportScanReport() {
  if (!state.lastScanResult) {
    window.alert("Run a scan first so there is data to export.");
    return;
  }

  try {
    const payload = {
      generatedAt: new Date().toISOString(),
      settings: {
        includeAdvanced: state.includeAdvanced,
        deleteMode: state.deleteMode,
      },
      selectedIds: [...state.selectedIds],
      scan: state.lastScanResult,
    };

    const result = await window.cleanerApi.exportReport({ report: payload });
    if (result.saved) {
      appendLog(`Scan report exported to: ${result.filePath}`);
    } else {
      appendLog("Scan report export canceled.");
    }
  } catch (error) {
    appendLog(`Export failed: ${error.message}`);
  }
}

function registerProgressEvents() {
  window.cleanerApi.onScanProgress((event) => {
    setStatus(`Scanning ${event.step}/${event.total}: ${event.categoryTitle}`);
  });

  window.cleanerApi.onCleanProgress((event) => {
    setStatus(`Cleaning ${event.step}/${event.total}: ${event.categoryTitle}`);
  });
}

function bindEvents() {
  elements.scanBtn.addEventListener("click", runScan);
  elements.cleanBtn.addEventListener("click", runClean);
  elements.exportBtn.addEventListener("click", exportScanReport);

  elements.advancedToggle.addEventListener("change", (event) => {
    state.includeAdvanced = event.target.checked;
    renderCategories();
  });

  elements.recycleModeToggle.addEventListener("change", (event) => {
    state.deleteMode = event.target.checked ? "recycle" : "permanent";
    appendLog(`Delete mode set to ${state.deleteMode}.`);
  });

  elements.onlyDetectedToggle.addEventListener("change", (event) => {
    state.showOnlyDetected = event.target.checked;
    renderCategories();
  });

  elements.searchInput.addEventListener("input", (event) => {
    state.searchQuery = event.target.value;
    renderCategories();
  });

  elements.sortSelect.addEventListener("change", (event) => {
    state.sortMode = event.target.value;
    renderCategories();
  });

  elements.presetRecommendedBtn.addEventListener("click", selectRecommended);
  elements.presetDeepBtn.addEventListener("click", selectDeepClean);
  elements.presetAggressiveBtn.addEventListener("click", selectAggressive);
  elements.selectTopSpaceBtn.addEventListener("click", selectTopSpaceHogs);
  elements.selectRecommendedBtn.addEventListener("click", selectRecommended);
  elements.selectVisibleBtn.addEventListener("click", selectVisible);
  elements.clearSelectionBtn.addEventListener("click", clearSelection);
}

async function bootstrap() {
  registerProgressEvents();
  bindEvents();
  await loadCategories();
  await loadSystemOverview();
  renderCategories();
  renderTopWaste();
  updateTopMetrics();
}

bootstrap().catch((error) => {
  appendLog(`Startup failure: ${error.message}`);
  setStatus("Startup failure");
});
