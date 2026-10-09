// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Max Boksem. See NOTICE for additional terms under GPLv3 section 7.

import { openUrl } from "@/lib/openUrl";

export const REPO_URL = "https://github.com/MaxMB15/MaxVideoPlayer";
export const LICENSE_URL = `${REPO_URL}/blob/main/LICENSE`;
export const NOTICE_URL = `${REPO_URL}/blob/main/NOTICE`;
export const THIRD_PARTY_URL = `${REPO_URL}/blob/main/THIRD_PARTY_NOTICES.md`;

// Each release is tagged, so the tag holds the exact source of the running build.
export const sourceUrl = (version?: string): string =>
	version ? `${REPO_URL}/tree/v${version}` : REPO_URL;

interface LegalNoticesProps {
	version?: string;
}

// The Appropriate Legal Notices that GPLv3 asks interactive programs to show.
// The "Created by" line is the attribution that NOTICE term 1 protects.
export const LegalNotices = ({ version }: LegalNoticesProps) => {
	const links = [
		{ label: "License", url: LICENSE_URL },
		{ label: "Additional terms", url: NOTICE_URL },
		{ label: "Source code", url: sourceUrl(version) },
		{ label: "Third-party notices", url: THIRD_PARTY_URL },
	];

	return (
		<div className="space-y-2 text-xs text-muted-foreground">
			<p className="text-sm text-foreground">Created by Max Boksem</p>
			<p>Copyright © 2026 Max Boksem</p>
			<p>
				Max Video Player is free software. You can share and change it under the GNU General
				Public License version 3, with additional terms. It comes with no warranty.
			</p>
			<div className="flex flex-wrap gap-x-3 gap-y-1">
				{links.map(({ label, url }) => (
					<button
						key={label}
						type="button"
						onClick={() => openUrl(url)}
						className="text-primary hover:underline"
					>
						{label}
					</button>
				))}
			</div>
		</div>
	);
};
