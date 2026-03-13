import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Loader2, CalendarDays, Clock } from "lucide-react";
import type { EpgProgram } from "@/lib/types";

export const ProgramGuide = () => {
	const [epgUrl, setEpgUrl] = useState("");
	const [programs, setPrograms] = useState<EpgProgram[]>([]);
	const [loading, setLoading] = useState(false);
	const [loaded, setLoaded] = useState(false);

	const handleLoadEpg = async () => {
		if (!epgUrl) return;
		setLoading(true);
		try {
			// Will be wired to Tauri command once backend EPG loading is connected
			setPrograms([]);
			setLoaded(true);
		} finally {
			setLoading(false);
		}
	};

	return (
		<div className="flex flex-col gap-6 p-4 h-full">
			<div className="flex items-center gap-3">
				<CalendarDays className="h-6 w-6 text-primary" />
				<h1 className="text-2xl font-bold">Program Guide</h1>
			</div>

			<Card>
				<CardHeader className="py-4">
					<CardTitle className="text-base">EPG Source</CardTitle>
				</CardHeader>
				<CardContent className="pb-4">
					<div className="flex gap-2">
						<Input
							placeholder="XMLTV EPG URL"
							value={epgUrl}
							onChange={(e) => setEpgUrl(e.target.value)}
						/>
						<Button onClick={handleLoadEpg} disabled={loading || !epgUrl}>
							{loading && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
							Load
						</Button>
					</div>
				</CardContent>
			</Card>

			{loaded && programs.length === 0 && (
				<div className="flex flex-col items-center justify-center flex-1 text-muted-foreground">
					<Clock className="h-12 w-12 mb-3 opacity-50" />
					<p>No program data available</p>
					<p className="text-sm">Load an XMLTV EPG URL to see the program guide</p>
				</div>
			)}

			{programs.length > 0 && (
				<ScrollArea className="flex-1">
					<div className="space-y-2">
						{programs.map((prog, i) => (
							<Card key={i}>
								<CardContent className="p-3">
									<div className="flex items-start justify-between">
										<div>
											<p className="font-medium text-sm">{prog.title}</p>
											<p className="text-xs text-muted-foreground mt-1">
												{prog.description}
											</p>
										</div>
										<div className="text-right shrink-0 ml-4">
											<p className="text-xs text-muted-foreground">
												{prog.startTime} - {prog.endTime}
											</p>
											{prog.category && (
												<span className="text-[10px] px-1.5 py-0.5 rounded bg-secondary text-secondary-foreground">
													{prog.category}
												</span>
											)}
										</div>
									</div>
								</CardContent>
							</Card>
						))}
					</div>
				</ScrollArea>
			)}
		</div>
	);
};
