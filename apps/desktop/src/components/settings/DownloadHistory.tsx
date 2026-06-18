import { useDownloads } from "@/hooks/useDownloads";
import type { DownloadRecord } from "@/lib/types";

const fmtBytes = (b: number | null) => (b == null ? "—" : `${(b / 1_000_000).toFixed(0)} MB`);
const fmtRate = (b: number | null) => (b == null ? "—" : `${(b / 1_000_000).toFixed(1)} MB/s`);
const fmtTime = (t: number | null) => (t == null ? "—" : new Date(t * 1000).toLocaleString());

const statusCell = (d: DownloadRecord) => {
	if (d.status === "completed") return <span className="text-emerald-500">Done</span>;
	if (d.status === "failed")
		return <span className="text-red-400">Failed{d.error ? ` — ${d.error}` : ""}</span>;
	if (d.status === "downloading") {
		const pct = d.totalBytes ? Math.round((d.downloadedBytes / d.totalBytes) * 100) : null;
		return <span className="text-blue-400">{pct == null ? "…" : `${pct}%`}</span>;
	}
	return <span className="text-muted-foreground">{d.status}</span>;
};

export const DownloadHistory = () => {
	const { downloads } = useDownloads();

	if (downloads.length === 0) {
		return <p className="text-xs text-muted-foreground">No downloads yet.</p>;
	}

	return (
		<div className="overflow-x-auto">
			<table className="w-full text-xs">
				<thead className="text-muted-foreground">
					<tr>
						<th className="text-left py-2">Title</th>
						<th className="text-left">Type</th>
						<th className="text-left">Size</th>
						<th className="text-left">Avg rate</th>
						<th className="text-left">Started</th>
						<th className="text-left">Finished</th>
						<th className="text-left">Status</th>
					</tr>
				</thead>
				<tbody>
					{downloads.map((d) => (
						<tr key={d.id} className="border-t border-border/40">
							<td className="py-2 pr-2 max-w-[180px] truncate">{d.title}</td>
							<td className="pr-2">{d.kind}</td>
							<td className="pr-2 whitespace-nowrap">
								{d.totalBytes
									? `${fmtBytes(d.downloadedBytes)} / ${fmtBytes(d.totalBytes)}`
									: fmtBytes(d.downloadedBytes)}
							</td>
							<td className="pr-2 whitespace-nowrap">{fmtRate(d.avgRateBps)}</td>
							<td className="pr-2 whitespace-nowrap">{fmtTime(d.createdAt)}</td>
							<td className="pr-2 whitespace-nowrap">{fmtTime(d.finishedAt)}</td>
							<td>{statusCell(d)}</td>
						</tr>
					))}
				</tbody>
			</table>
		</div>
	);
};
