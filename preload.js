const { contextBridge, ipcRenderer } = require("electron");

function addListener(channel, callback) {
  const listener = (_, payload) => callback(payload);
  ipcRenderer.on(channel, listener);
  return () => {
    ipcRenderer.removeListener(channel, listener);
  };
}

contextBridge.exposeInMainWorld("cleanerApi", {
  getCategories: () => ipcRenderer.invoke("cleaner:getCategories"),
  getSystemOverview: () => ipcRenderer.invoke("cleaner:getSystemOverview"),
  scan: (payload) => ipcRenderer.invoke("cleaner:scan", payload),
  clean: (payload) => ipcRenderer.invoke("cleaner:clean", payload),
  exportReport: (payload) => ipcRenderer.invoke("cleaner:exportReport", payload),
  onScanProgress: (callback) => addListener("cleaner:scan-progress", callback),
  onCleanProgress: (callback) => addListener("cleaner:clean-progress", callback),
});
