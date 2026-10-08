import { describe, it, expect, beforeEach } from "vitest";
import {
	consumeBrowseState,
	markWatchEnded,
	markWatchStarted,
	saveBrowseSearch,
	saveBrowseTab,
	SEARCH_RESET_AFTER_MS,
} from "./browse-state";

describe("browse-state", () => {
	beforeEach(() => sessionStorage.clear());

	it("returns defaults when nothing is stored", () => {
		expect(consumeBrowseState()).toEqual({ tab: undefined, search: "" });
	});

	it("restores tab and search", () => {
		saveBrowseTab("movie");
		saveBrowseSearch("matrix");
		expect(consumeBrowseState()).toEqual({ tab: "movie", search: "matrix" });
	});

	it("keeps the search after a short watch", () => {
		saveBrowseSearch("matrix");
		markWatchStarted(1_000);
		markWatchEnded(1_000 + SEARCH_RESET_AFTER_MS - 1);
		expect(consumeBrowseState(10_000_000).search).toBe("matrix");
	});

	it("clears the search after watching for a minute", () => {
		saveBrowseSearch("matrix");
		markWatchStarted(1_000);
		markWatchEnded(1_000 + SEARCH_RESET_AFTER_MS);
		expect(consumeBrowseState().search).toBe("");
		// The cleared search sticks.
		expect(consumeBrowseState().search).toBe("");
	});

	it("measures up to now when the player hasn't recorded an end yet", () => {
		saveBrowseSearch("matrix");
		markWatchStarted(1_000);
		expect(consumeBrowseState(1_000 + SEARCH_RESET_AFTER_MS).search).toBe("");
	});

	it("ends the session once consumed", () => {
		saveBrowseSearch("matrix");
		markWatchStarted(1_000);
		markWatchEnded(2_000);
		consumeBrowseState();
		saveBrowseSearch("other");
		// No session anymore, so a late markWatchEnded is ignored and the search stays.
		markWatchEnded(1_000_000);
		expect(consumeBrowseState(1_000_000).search).toBe("other");
	});

	it("a session continues across player visits", () => {
		saveBrowseSearch("matrix");
		markWatchStarted(1_000);
		markWatchEnded(2_000);
		markWatchStarted(50_000); // back in the player without visiting the list
		markWatchEnded(1_000 + SEARCH_RESET_AFTER_MS);
		expect(consumeBrowseState().search).toBe("");
	});
});
