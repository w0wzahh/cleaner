const fs = require("fs/promises");
const fssync = require("fs");
const path = require("path");
const { execFile } = require("child_process");
const os = require("os");
const { CLEANER_CATEGORIES } = require("./categories");

const MAX_SCAN_ENTRIES = 250000;

function resolveEnvPath(input) {
  if (!input || typeof input !== "string") {
    return input;
  }

  return input.replace(/%([^%]+)%/g, (_, key) => process.env[key] || `%${key}%`);
}

function assertPathIsSafe(inputPath) {
  const resolved = path.resolve(inputPath);
  const parsed = path.parse(resolved);

  if (resolved === parsed.root) {
    throw new Error(`Refusing to operate on drive root: ${resolved}`);
  }

  if (resolved.length < 4) {
    throw new Error(`Refusing to operate on suspicious path: ${resolved}`);
  }
}

function categoryVisible(category, includeAdvanced) {
  return includeAdvanced ? true : !category.advanced;
}

function pickCategories({ includeAdvanced = false, ids }) {
  const allVisible = CLEANER_CATEGORIES.filter((category) =>
    categoryVisible(category, includeAdvanced)
  );

  if (!Array.isArray(ids) || ids.length === 0) {
    return allVisible;
  }

  const idSet = new Set(ids);
  return CLEANER_CATEGORIES.filter((category) => idSet.has(category.id));
}

function summarizeOperationTargets(operations) {
  const paths = [];

  for (const operation of operations) {
    if (operation.path) {
      const resolved = resolveEnvPath(operation.path);
      paths.push(resolved);
    }
  }

  return [...new Set(paths)];
}

function getCategoryCatalog() {
  return CLEANER_CATEGORIES.map((category) => ({
    id: category.id,
    title: category.title,
    description: category.description,
    whyItHelps: category.whyItHelps,
    risk: category.risk,
    advanced: category.advanced,
    safeDefault: category.safeDefault,
    targets: summarizeOperationTargets(category.operations),
  }));
}

function pathExists(targetPath) {
  return fssync.existsSync(targetPath);
}

async function scanSingleFile(targetPath) {
  try {
    if (!pathExists(targetPath)) {
      return { bytes: 0, files: 0, issues: [] };
    }

    const stats = await fs.stat(targetPath);
    if (!stats.isFile()) {
      return { bytes: 0, files: 0, issues: [] };
    }

    return { bytes: stats.size, files: 1, issues: [] };
  } catch (error) {
    return { bytes: 0, files: 0, issues: [`${targetPath}: ${error.message}`] };
  }
}

async function scanDirectory(
  targetPath,
  { recursive = true, filePattern = null, maxEntries = MAX_SCAN_ENTRIES } = {}
) {
  const issues = [];

  if (!pathExists(targetPath)) {
    return { bytes: 0, files: 0, issues };
  }

  const matcher = filePattern ? new RegExp(filePattern, "i") : null;
  let bytes = 0;
  let files = 0;
  let visited = 0;
  const stack = [targetPath];

  while (stack.length > 0) {
    const currentDir = stack.pop();
    let dir;

    try {
      dir = await fs.opendir(currentDir);
    } catch (error) {
      issues.push(`${currentDir}: ${error.message}`);
      continue;
    }

    try {
      for await (const entry of dir) {
        visited += 1;
        if (visited > maxEntries) {
          issues.push(`Scan limit reached in ${targetPath} at ${maxEntries} entries.`);
          return { bytes, files, issues };
        }

        const fullPath = path.join(currentDir, entry.name);

        if (entry.isSymbolicLink()) {
          continue;
        }

        if (entry.isDirectory()) {
          if (recursive) {
            stack.push(fullPath);
          }
          continue;
        }

        if (!entry.isFile()) {
          continue;
        }

        if (matcher && !matcher.test(entry.name)) {
          continue;
        }

        try {
          const stats = await fs.stat(fullPath);
          bytes += stats.size;
          files += 1;
        } catch (error) {
          issues.push(`${fullPath}: ${error.message}`);
        }
      }
    } catch (error) {
      issues.push(`${currentDir}: ${error.message}`);
    }
  }

  return { bytes, files, issues };
}

async function scanRecycleBin() {
  let totalBytes = 0;
  let totalFiles = 0;
  const issues = [];

  const driveLetters = "CDEFGHIJKLMNOPQRSTUVWXYZ".split("");

  for (const letter of driveLetters) {
    const recyclePath = `${letter}:\\$Recycle.Bin`;
    if (!pathExists(recyclePath)) {
      continue;
    }

    const result = await scanDirectory(recyclePath, { recursive: true });
    totalBytes += result.bytes;
    totalFiles += result.files;
    issues.push(...result.issues);
  }

  return { bytes: totalBytes, files: totalFiles, issues };
}

function runCommand(shell, command) {
  return new Promise((resolve) => {
    const options = {
      windowsHide: true,
      maxBuffer: 1024 * 1024 * 10,
    };

    let executable;
    let args;

    if (shell === "powershell") {
      executable = "powershell.exe";
      args = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command];
    } else {
      executable = "cmd.exe";
      args = ["/d", "/s", "/c", command];
    }

    execFile(executable, args, options, (error, stdout, stderr) => {
      resolve({
        ok: !error,
        stdout: stdout ? stdout.trim() : "",
        stderr: stderr ? stderr.trim() : "",
        error: error ? error.message : null,
      });
    });
  });
}

function escapePowerShellSingleQuoted(input) {
  return String(input).replace(/'/g, "''");
}

async function movePathToRecycleBin(targetPath) {
  const escapedPath = escapePowerShellSingleQuoted(targetPath);
  const command = [
    "$ErrorActionPreference='Stop'",
    "Add-Type -AssemblyName Microsoft.VisualBasic",
    `$target='${escapedPath}'`,
    "if (!(Test-Path -LiteralPath $target)) { return }",
    "$item = Get-Item -LiteralPath $target -Force",
    "if ($item.PSIsContainer) {",
    "  [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($target, [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs, [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin)",
    "} else {",
    "  [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($target, [Microsoft.VisualBasic.FileIO.UIOption]::OnlyErrorDialogs, [Microsoft.VisualBasic.FileIO.RecycleOption]::SendToRecycleBin)",
    "}",
  ].join("; ");

  return runCommand("powershell", command);
}

async function scanDirectoryByChildSubdirs(basePath, childDirNames) {
  const issues = [];
  let bytes = 0;
  let files = 0;

  if (!pathExists(basePath)) {
    return { bytes, files, issues };
  }

  let children = [];
  try {
    children = await fs.readdir(basePath, { withFileTypes: true });
  } catch (error) {
    return { bytes, files, issues: [`${basePath}: ${error.message}`] };
  }

  for (const child of children) {
    if (!child.isDirectory()) {
      continue;
    }

    for (const subdirName of childDirNames) {
      const targetPath = path.join(basePath, child.name, subdirName);
      const result = await scanDirectory(targetPath, { recursive: true });
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
    }
  }

  return { bytes, files, issues };
}

async function cleanDirectoryByChildSubdirs(basePath, childDirNames, deleteMode) {
  const issues = [];
  let deleted = 0;

  if (!pathExists(basePath)) {
    return { issues, deleted };
  }

  let children = [];
  try {
    children = await fs.readdir(basePath, { withFileTypes: true });
  } catch (error) {
    return { issues: [`${basePath}: ${error.message}`], deleted };
  }

  for (const child of children) {
    if (!child.isDirectory()) {
      continue;
    }

    for (const subdirName of childDirNames) {
      const targetPath = path.join(basePath, child.name, subdirName);
      const result = await cleanDirectoryContents(targetPath, { deleteMode });
      issues.push(...result.issues);
      deleted += result.deleted;
    }
  }

  return { issues, deleted };
}

async function cleanDirectoryContents(targetPath, { deleteMode = "permanent" } = {}) {
  const issues = [];
  let deleted = 0;

  if (!pathExists(targetPath)) {
    return { issues, deleted };
  }

  assertPathIsSafe(targetPath);

  let entries;
  try {
    entries = await fs.readdir(targetPath);
  } catch (error) {
    return {
      issues: [`${targetPath}: ${error.message}`],
      deleted,
    };
  }

  for (const entry of entries) {
    const fullPath = path.join(targetPath, entry);
    try {
      if (deleteMode === "recycle") {
        const recycleResult = await movePathToRecycleBin(fullPath);
        if (!recycleResult.ok) {
          issues.push(`${fullPath}: ${recycleResult.error || recycleResult.stderr || "Recycle bin move failed"}`);
        } else {
          deleted += 1;
        }
      } else {
        await fs.rm(fullPath, { recursive: true, force: true, maxRetries: 1 });
        deleted += 1;
      }
    } catch (error) {
      issues.push(`${fullPath}: ${error.message}`);
    }
  }

  return { issues, deleted };
}

async function cleanDirectoryMatchingFiles(targetPath, filePattern, { deleteMode = "permanent" } = {}) {
  const issues = [];
  let deleted = 0;

  if (!pathExists(targetPath)) {
    return { issues, deleted };
  }

  assertPathIsSafe(targetPath);
  const matcher = new RegExp(filePattern, "i");

  let entries;
  try {
    entries = await fs.readdir(targetPath, { withFileTypes: true });
  } catch (error) {
    return {
      issues: [`${targetPath}: ${error.message}`],
      deleted,
    };
  }

  for (const entry of entries) {
    if (!entry.isFile() || !matcher.test(entry.name)) {
      continue;
    }

    const fullPath = path.join(targetPath, entry.name);

    try {
      if (deleteMode === "recycle") {
        const recycleResult = await movePathToRecycleBin(fullPath);
        if (!recycleResult.ok) {
          issues.push(`${fullPath}: ${recycleResult.error || recycleResult.stderr || "Recycle bin move failed"}`);
        } else {
          deleted += 1;
        }
      } else {
        await fs.rm(fullPath, { force: true });
        deleted += 1;
      }
    } catch (error) {
      issues.push(`${fullPath}: ${error.message}`);
    }
  }

  return { issues, deleted };
}

async function cleanSingleFile(targetPath, { deleteMode = "permanent" } = {}) {
  if (!pathExists(targetPath)) {
    return { issues: [], deleted: 0 };
  }

  assertPathIsSafe(targetPath);

  try {
    if (deleteMode === "recycle") {
      const recycleResult = await movePathToRecycleBin(targetPath);
      if (!recycleResult.ok) {
        return {
          issues: [
            `${targetPath}: ${
              recycleResult.error || recycleResult.stderr || "Recycle bin move failed"
            }`,
          ],
          deleted: 0,
        };
      }
      return { issues: [], deleted: 1 };
    }

    await fs.rm(targetPath, { force: true });
    return { issues: [], deleted: 1 };
  } catch (error) {
    return {
      issues: [`${targetPath}: ${error.message}`],
      deleted: 0,
    };
  }
}

async function scanCategory(category) {
  let bytes = 0;
  let files = 0;
  const issues = [];

  for (const operation of category.operations) {
    if (operation.type === "directoryContents") {
      const result = await scanDirectory(resolveEnvPath(operation.path), {
        recursive: true,
      });
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
      continue;
    }

    if (operation.type === "directoryFilesByPattern") {
      const result = await scanDirectory(resolveEnvPath(operation.path), {
        recursive: false,
        filePattern: operation.pattern,
      });
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
      continue;
    }

    if (operation.type === "file") {
      const result = await scanSingleFile(resolveEnvPath(operation.path));
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
      continue;
    }

    if (operation.type === "profileSubdirectories") {
      const result = await scanDirectoryByChildSubdirs(
        resolveEnvPath(operation.path),
        Array.isArray(operation.childDirectories) ? operation.childDirectories : []
      );
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
      continue;
    }

    if (operation.type === "recycleBin") {
      const result = await scanRecycleBin();
      bytes += result.bytes;
      files += result.files;
      issues.push(...result.issues);
    }
  }

  return { bytes, files, issues };
}

async function cleanCategory(category, { deleteMode = "permanent" } = {}) {
  const issues = [];
  const actions = [];

  for (const operation of category.operations) {
    if (operation.type === "directoryContents") {
      const targetPath = resolveEnvPath(operation.path);
      const result = await cleanDirectoryContents(targetPath, { deleteMode });
      issues.push(...result.issues);
      actions.push(
        `${deleteMode === "recycle" ? "Moved" : "Deleted"} ${result.deleted} entries from ${targetPath}`
      );
      continue;
    }

    if (operation.type === "directoryFilesByPattern") {
      const targetPath = resolveEnvPath(operation.path);
      const result = await cleanDirectoryMatchingFiles(targetPath, operation.pattern, {
        deleteMode,
      });
      issues.push(...result.issues);
      actions.push(
        `${deleteMode === "recycle" ? "Moved" : "Deleted"} ${result.deleted} matching files from ${targetPath}`
      );
      continue;
    }

    if (operation.type === "file") {
      const targetPath = resolveEnvPath(operation.path);
      const result = await cleanSingleFile(targetPath, { deleteMode });
      issues.push(...result.issues);
      actions.push(`${deleteMode === "recycle" ? "Moved" : "Deleted"} ${result.deleted} file(s): ${targetPath}`);
      continue;
    }

    if (operation.type === "profileSubdirectories") {
      const basePath = resolveEnvPath(operation.path);
      const result = await cleanDirectoryByChildSubdirs(
        basePath,
        Array.isArray(operation.childDirectories) ? operation.childDirectories : [],
        deleteMode
      );
      issues.push(...result.issues);
      actions.push(
        `${deleteMode === "recycle" ? "Moved" : "Deleted"} ${result.deleted} profile cache entries under ${basePath}`
      );
      continue;
    }

    if (operation.type === "recycleBin") {
      const recycleCommand = [
        "$ErrorActionPreference='Stop'",
        "if (Get-Command Clear-RecycleBin -ErrorAction SilentlyContinue) {",
        "  Clear-RecycleBin -Force -Confirm:$false -ErrorAction Stop",
        "} else {",
        "  $shell = New-Object -ComObject Shell.Application",
        "  $bin = $shell.Namespace(0xA)",
        "  if ($null -ne $bin) {",
        "    $items = @($bin.Items())",
        "    foreach ($item in $items) { $item.InvokeVerb('delete') }",
        "  }",
        "}",
      ].join("; ");

      const cmdResult = await runCommand("powershell", recycleCommand);
      if (!cmdResult.ok) {
        issues.push(`Recycle Bin cleanup failed: ${cmdResult.error || cmdResult.stderr}`);
      }
      actions.push("Requested Recycle Bin cleanup across available drives");
      continue;
    }

    if (operation.type === "command") {
      const cmdResult = await runCommand(operation.shell || "cmd", operation.command);
      if (!cmdResult.ok) {
        issues.push(
          `${category.title} command failed: ${cmdResult.error || cmdResult.stderr || "Unknown error"}`
        );
      }
      if (operation.note) {
        actions.push(operation.note);
      }
      if (cmdResult.stdout) {
        actions.push(cmdResult.stdout);
      }
      if (cmdResult.stderr) {
        actions.push(cmdResult.stderr);
      }
    }
  }

  return {
    actions,
    issues,
  };
}

async function scanCategories({ includeAdvanced = false, ids, sendProgress = null } = {}) {
  const categories = pickCategories({ includeAdvanced, ids });
  const results = [];
  let totalBytes = 0;
  let totalFiles = 0;

  for (let index = 0; index < categories.length; index += 1) {
    const category = categories[index];

    if (sendProgress) {
      sendProgress({
        phase: "scan",
        step: index + 1,
        total: categories.length,
        categoryId: category.id,
        categoryTitle: category.title,
      });
    }

    const scan = await scanCategory(category);

    totalBytes += scan.bytes;
    totalFiles += scan.files;

    results.push({
      id: category.id,
      title: category.title,
      risk: category.risk,
      advanced: category.advanced,
      bytes: scan.bytes,
      files: scan.files,
      issues: scan.issues,
    });
  }

  return {
    scannedAt: new Date().toISOString(),
    totalBytes,
    totalFiles,
    results,
  };
}

async function cleanCategories({ ids, deleteMode = "permanent", sendProgress = null } = {}) {
  if (!Array.isArray(ids) || ids.length === 0) {
    throw new Error("No categories selected.");
  }

  const safeDeleteMode = ["permanent", "recycle"].includes(deleteMode)
    ? deleteMode
    : "permanent";

  const categories = pickCategories({ includeAdvanced: true, ids });
  const results = [];
  let totalReclaimedBytes = 0;

  for (let index = 0; index < categories.length; index += 1) {
    const category = categories[index];

    if (sendProgress) {
      sendProgress({
        phase: "clean",
        step: index + 1,
        total: categories.length,
        categoryId: category.id,
        categoryTitle: category.title,
      });
    }

    const before = await scanCategory(category);
    const cleanResult = await cleanCategory(category, { deleteMode: safeDeleteMode });
    const after = await scanCategory(category);

    const reclaimedBytes = Math.max(0, before.bytes - after.bytes);
    totalReclaimedBytes += reclaimedBytes;

    results.push({
      id: category.id,
      title: category.title,
      risk: category.risk,
      reclaimedBytes,
      beforeBytes: before.bytes,
      afterBytes: after.bytes,
      actions: cleanResult.actions,
      issues: [...before.issues, ...cleanResult.issues, ...after.issues],
      success: cleanResult.issues.length === 0,
    });
  }

  return {
    cleanedAt: new Date().toISOString(),
    deleteMode: safeDeleteMode,
    totalReclaimedBytes,
    results,
  };
}

async function getSystemOverview() {
  const cpus = os.cpus();
  const memoryTotal = os.totalmem();
  const memoryFree = os.freemem();
  const uptimeSeconds = os.uptime();

  const diskCommand = [
    "$drive = Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='C:'\"",
    "if ($null -eq $drive) { Write-Output '{}' ; exit 0 }",
    "$obj = [PSCustomObject]@{ free = [int64]$drive.FreeSpace; size = [int64]$drive.Size }",
    "$obj | ConvertTo-Json -Compress",
  ].join("; ");

  const diskResult = await runCommand("powershell", diskCommand);
  let disk = { free: 0, size: 0 };

  if (diskResult.ok && diskResult.stdout) {
    try {
      disk = JSON.parse(diskResult.stdout);
    } catch {
      disk = { free: 0, size: 0 };
    }
  }

  return {
    host: os.hostname(),
    user: os.userInfo().username,
    cpuModel: cpus.length > 0 ? cpus[0].model : "Unknown",
    cpuCores: cpus.length,
    memoryTotal,
    memoryFree,
    uptimeSeconds,
    systemDrive: disk,
  };
}

module.exports = {
  getCategoryCatalog,
  scanCategories,
  cleanCategories,
  getSystemOverview,
};
