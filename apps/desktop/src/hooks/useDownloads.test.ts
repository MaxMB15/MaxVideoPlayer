import { describe, it, expect } from "vitest";
import { aggregateForSeries } from "./useDownloads";
import type { DownloadRecord } from "@/lib/types";

const rec = (over: Partial<DownloadRecord>): DownloadRecord => ({
	id: "x",
	channelId: "x",
	title: "t",
	kind: "episode",
	seriesChannelId: "S1",
	status: "completed",
	destPath: "/p",
	totalBytes: null,
	downloadedBytes: 0,
	avgRateBps: null,
	error: null,
	createdAt: 0,
	finishedAt: null,
	...over,
});

describe("aggregateForSeries", () => {
	it("returns none when no episodes downloaded", () => {
		expect(aggregateForSeries([], 24)).toBe("none");
	});
	it("returns downloading when any in progress", () => {
		const recs = [rec({ status: "downloading" }), rec({ status: "completed" })];
		expect(aggregateForSeries(recs, 24)).toBe("downloading");
	});
	it("returns partial when some but not all complete", () => {
		const recs = [rec({ status: "completed" }), rec({ status: "completed" })];
		expect(aggregateForSeries(recs, 24)).toBe("partial");
	});
	it("returns complete when all episodes downloaded", () => {
		const recs = Array.from({ length: 24 }, () => rec({ status: "completed" }));
		expect(aggregateForSeries(recs, 24)).toBe("complete");
	});
});
