import { workspace, window, type ExtensionContext } from "vscode";
import {
  LanguageClient,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node.js";

import { resolveBinary } from "./binary.js";
import { buildInitializationOptions } from "./config.js";

let client: LanguageClient | undefined;

async function startClient(): Promise<void> {
  const binary = resolveBinary();
  if (!binary) {
    window.showErrorMessage(
      "css-var-kit: could not find the `cvk` binary. " +
        "Install `css-var-kit` via npm, add `cvk` to your PATH, or set `cvk.path` in settings.",
    );
    return;
  }

  const serverOptions: ServerOptions = {
    command: binary,
    args: ["lsp"],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "css" }],
    initializationOptions: buildInitializationOptions(),
  };

  client = new LanguageClient("cvk", "CSS Var Kit", serverOptions, clientOptions);
  await client.start();
}

async function restartClient(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
  await startClient();
}

export async function activate(context: ExtensionContext): Promise<void> {
  await startClient();

  context.subscriptions.push(
    workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("cvk")) {
        void restartClient();
      }
    }),
  );
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
