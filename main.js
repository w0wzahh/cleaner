const path = require("path");
const fs = require("fs/promises");
const { app, BrowserWindow, ipcMain, dialog } = require("electron");
const {
  getCategoryCatalog,
  scanCategories,
  cleanCategories,
  getSystemOverview,
} = require("./src/cleaner/windowsCleaner");

function createWindow() {
  const win = new BrowserWindow({
    width: 1240,
    height: 820,
    minWidth: 1024,
    minHeight: 700,
    backgroundColor: "#0a1222",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  win.loadFile(path.join(__dirname, "renderer", "index.html"));
}

app.whenReady().then(() => {
  ipcMain.handle("cleaner:getCategories", async () => {
    return getCategoryCatalog();
  });

  ipcMain.handle("cleaner:getSystemOverview", async () => {
    return getSystemOverview();
  });

  ipcMain.handle("cleaner:scan", async (event, payload = {}) => {
    return scanCategories({
      includeAdvanced: Boolean(payload.includeAdvanced),
      ids: Array.isArray(payload.ids) ? payload.ids : undefined,
      sendProgress: (progress) => event.sender.send("cleaner:scan-progress", progress),
    });
  });

  ipcMain.handle("cleaner:clean", async (event, payload = {}) => {
    if (!Array.isArray(payload.ids) || payload.ids.length === 0) {
      throw new Error("No categories selected for cleanup.");
    }

    return cleanCategories({
      ids: payload.ids,
      deleteMode: payload.deleteMode,
      sendProgress: (progress) => event.sender.send("cleaner:clean-progress", progress),
    });
  });

  ipcMain.handle("cleaner:exportReport", async (event, payload = {}) => {
    const report = payload.report;
    if (!report || typeof report !== "object") {
      throw new Error("No report data to export.");
    }

    const defaultName = `deepspace-scan-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
    const win = BrowserWindow.fromWebContents(event.sender);

    const save = await dialog.showSaveDialog(win, {
      title: "Export Scan Report",
      defaultPath: path.join(app.getPath("documents"), defaultName),
      filters: [{ name: "JSON", extensions: ["json"] }],
    });

    if (save.canceled || !save.filePath) {
      return { saved: false };
    }

    await fs.writeFile(save.filePath, JSON.stringify(report, null, 2), "utf8");
    return { saved: true, filePath: save.filePath };
  });

  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
