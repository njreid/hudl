import * as path from 'path';
import { workspace, ExtensionContext } from 'vscode';
import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
	// The server is implemented in Rust
	// We assume 'hudl-lsp' is in the system PATH or provided in the extension
	let serverExecutable = 'hudl-lsp';

	let serverOptions: ServerOptions = {
		run: { command: serverExecutable, transport: TransportKind.stdio },
		debug: { command: serverExecutable, transport: TransportKind.stdio }
	};

	let clientOptions: LanguageClientOptions = {
		documentSelector: [{ scheme: 'file', language: 'hudl' }],
		synchronize: {
			fileEvents: workspace.createFileSystemWatcher('**/*.hu.kdl')
		}
	};

	client = new LanguageClient(
		'hudlLanguageServer',
		'Hudl Language Server',
		serverOptions,
		clientOptions
	);

	client.start();
}

export function deactivate(): Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
