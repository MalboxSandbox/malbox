export type TruncateResult = {
	head: string;
	tail: string;
	truncated: boolean;
};

export function truncateWithTail(
	value: string,
	maxChars: number,
	tailLength: number
): TruncateResult {
	if (value.length <= maxChars) return { head: value, tail: '', truncated: false };
	const headLength = maxChars - tailLength - 1;
	return { head: value.slice(0, headLength), tail: value.slice(-tailLength), truncated: true };
}
