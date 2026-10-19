import { getActivityCoverage } from "@/lib/api/activity";

/**
 * Returns the earliest captured timestamp (frames, audio or privacy
 * notices), or `null` on a fresh install with nothing captured yet, or
 * `{ error }` on a request failure. Callers check for the `error` key.
 */
export async function getStartDate(): Promise<Date | { error: string } | null> {
	try {
		const coverage = await getActivityCoverage();
		return coverage.first_timestamp ? new Date(coverage.first_timestamp) : null;
	} catch (e) {
		return {
			error: "errro occured",
		};
	}
}
