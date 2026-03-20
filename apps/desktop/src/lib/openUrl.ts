export async function openUrl(url: string): Promise<void> {
	try {
		const opener = await import("@tauri-apps/plugin-opener");
		await opener.openUrl(url);
	} catch {
		window.open(url, "_blank");
	}
}
