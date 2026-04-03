import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@/lib/tauri", () => ({
	getGroupHierarchy: vi.fn(),
	getPinnedGroups: vi.fn(),
	pinGroup: vi.fn(),
	unpinGroup: vi.fn(),
}));

import { useGroupHierarchy } from "./useGroupHierarchy";
import { getGroupHierarchy, getPinnedGroups } from "@/lib/tauri";

const mockGetHierarchy = vi.mocked(getGroupHierarchy);
const mockGetPinned = vi.mocked(getPinnedGroups);

describe("useGroupHierarchy", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockGetHierarchy.mockResolvedValue([
			{
				providerId: "p1",
				contentType: "live",
				groupName: "US: Sports",
				superCategory: "United States",
				sortOrder: 0,
				isUserOverride: false,
			},
			{
				providerId: "p1",
				contentType: "live",
				groupName: "US: News",
				superCategory: "United States",
				sortOrder: 100,
				isUserOverride: false,
			},
			{
				providerId: "p1",
				contentType: "live",
				groupName: "Misc",
				superCategory: null,
				sortOrder: 200,
				isUserOverride: false,
			},
		]);
		mockGetPinned.mockResolvedValue([]);
	});

	it("loads hierarchy and derives super-categories", async () => {
		const { result } = renderHook(() => useGroupHierarchy("p1", "live"));
		await waitFor(() => expect(result.current.loaded).toBe(true));

		expect(result.current.superCategories).toEqual(["United States"]);
		expect(result.current.topLevelGroups).toEqual(["Misc"]);
		expect(result.current.getGroupsForCategory("United States")).toEqual([
			"US: Sports",
			"US: News",
		]);
	});
});
