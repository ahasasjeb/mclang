// Mclang 的 VSCode 入口：启动 `mclang lsp` 并通过 LSP 接入语言特性。
const fs = require("fs");
const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

/** @type {LanguageClient | undefined} */
let client;

// 解析服务器可执行文件：显式配置优先，其次工作区里 `cargo build` 的产物，最后交给 PATH。
function resolveServerPath() {
  const configured = vscode.workspace.getConfiguration("mclang").get("server.path");
  if (typeof configured === "string" && configured.trim() !== "") {
    return configured.trim();
  }
  const executable = process.platform === "win32" ? "mclang.exe" : "mclang";
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    for (const profile of ["release", "debug"]) {
      const candidate = vscode.Uri.joinPath(folder.uri, "target", profile, executable).fsPath;
      if (fs.existsSync(candidate)) {
        return candidate;
      }
    }
  }
  return "mclang";
}

async function startClient() {
  const serverOptions = {
    command: resolveServerPath(),
    args: ["lsp"],
    transport: TransportKind.stdio,
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "mclang" }],
    outputChannelName: "Mclang 语言服务器",
  };
  const instance = new LanguageClient("mclang", "Mclang", serverOptions, clientOptions);
  await instance.start();
  client = instance;
}

async function stopClient() {
  if (!client) {
    return;
  }
  const instance = client;
  client = undefined;
  await instance.stop();
}

async function restartClient() {
  await stopClient();
  try {
    await startClient();
  } catch (error) {
    showStartupError(error);
  }
}

function showStartupError(error) {
  const reason = error instanceof Error ? error.message : String(error);
  const message = `无法启动 mclang 语言服务器：${reason}。请先构建编译器（cargo build --release），或设置 mclang.server.path。`;
  vscode.window
    .showErrorMessage(message, "打开设置", "重新加载窗口")
    .then((choice) => {
      if (choice === "打开设置") {
        vscode.commands.executeCommand("workbench.action.openSettings", "mclang.server.path");
      } else if (choice === "重新加载窗口") {
        vscode.commands.executeCommand("workbench.action.reloadWindow");
      }
    });
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("mclang.restartServer", () => restartClient()),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("mclang.server.path")) {
        restartClient();
      }
    })
  );
  startClient().catch(showStartupError);
}

function deactivate() {
  return stopClient();
}

module.exports = { activate, deactivate };
