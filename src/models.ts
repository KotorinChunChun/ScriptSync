/** UIが使用するバックエンド応答の型。保存形式の正本はRust側のdomain.rs。 */
export type Take = {
    trim_start?: number;
    trim_end?: number;
    script_text?: string;
    id: string;
    file: string;
    hash: string;
    status: string;
    duration: number;
    peak: number;
    warning: string;
};
export type Correction = {
    id: string;
    start: number;
    end: number;
    original: string;
    replacement: string;
    decision: string;
};
export type Variant = {
    id: string;
    label: string;
    text: string;
    file: string;
    hash: string;
    status: string;
    error: string;
    transcript: string;
    rating: Record<string, number | string | null> | null;
};
export type Run = {
    kind?: string;
    id: string;
    variants: Variant[];
    order: string[];
    revealed: boolean;
    stale?: boolean;
    reference_kana?: string;
};
export type Asr = {
    take_id?: string;
    status?: string;
    id: string;
    file: string;
    audio_hash: string;
    raw?: string;
    kana?: string;
    warning?: string;
    error?: string;
    words?: {
        text: string;
        start: number;
        end: number;
    }[];
};
export type Segment = {
    source_id?: string;
    kind?: number;
    hierarchy?: number[];
    source_text?: string;
    source_line?: number;
    take_notes?: Record<string, {
        memo: string;
        favorite: boolean;
        score?: number | null;
    }>;
    needs_review?: boolean;
    gain_db?: number;
    id: string;
    text: string;
    revision: number;
    takes: Take[];
    selected: string | null;
    kana: string;
    confirmed: boolean;
    mappings: {
        start: number;
        end: number;
        kana: string;
    }[];
    corrections: Correction[];
    trim_start: number;
    trim_end: number;
    gap: number;
    skip: boolean;
    comparisons: Run[];
    transcriptions?: Asr[];
};
export type Snapshot = {
    stop_scheduled?: boolean;
    recording_peaks?: number[];
    project: {
        rules?: {
            id: string;
            original: string;
            reading: string;
            context: string;
            dictionary: string;
            status: string;
            validations: Run[];
            source: Run;
        }[];
        batches?: {
            id: string;
            items: {
                action: string;
                payload: {
                    segment: string;
                };
                status: string;
                error: string;
            }[];
        }[];
        archived_segments?: Segment[];
        title: string;
        generation: number;
        segments: Segment[];
    } | null;
    root: string | null;
    busy: boolean;
    job: string;
    message: string;
    peak: number;
    elapsed: number;
};
