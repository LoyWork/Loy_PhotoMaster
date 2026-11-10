import { PhotoId, TagId, TagById, PhotoSectionId, PhotoSectionById, Settings, UiConfig, PhotoDetail, PhotoWork, ImportProgress, PhotoFilter, PhotoExportOptions, PhotoExportProgress, MetaData, ExifData, Photo, PhotoFolder} from 'app/CommonTypes'
import { FetchState } from 'app/UITypes'
import { SimilarModelStatus, SimilarScanMode, SimilarScanResult, SimilarSensitivity } from 'app/CommonTypes'


export type AppState = {
    navigation: NavigationState
    data: DataState
    library: LibraryState
    detail: DetailState
    info: InfoState
    import: ImportState
    export: ExportState
    photoCleanup: Record<PhotoCleanupKind, PhotoCleanupSession>
}

export type PhotoCleanupKind = 'similar' | 'duplicates'

/** In-memory workspace for each cleanup page; not persisted across app restarts. */
export interface PhotoCleanupSession {
    mode: SimilarScanMode
    model: SimilarModelStatus | null
    sensitivity: SimilarSensitivity
    result: SimilarScanResult | null
    selected: Set<PhotoId>
    page: number
    scrollTop: number
    expandedGroups: Record<number, number>
    hadGroups: boolean
    notice: string
}


export type NavigationState = {
    devicePixelRatio: number
    hasWebGLSupport: boolean
    isFullScreen: boolean
    isShiftPressed: boolean
    /** Whether the library's left sidebar (filters + folders) is shown. */
    showSidebar: boolean
    mainView: MainViewState
}

export type MainViewState = 'settings' | 'detail' | null


export type DataState = {
    readonly uiConfig: UiConfig
    readonly settings: Settings
    readonly tags: TagsState
    readonly folders: FoldersState
    readonly sections: SectionsState
}


export interface LibraryState {
    readonly display: DisplayState
    readonly filter: PhotoFilter
    /**
     * The active photo. This is the photo on which the last action was performed
     * (like viewing in detail, selecting or deseleting).
     * 
     * The active photo ...
     *   - acts as anchor for shift-(de)selection
     *   - has the keyboard focus
     */
    readonly activePhoto: PhotoLibraryPosition | null
    /** The photo over which the mouse is currently hovering */
    readonly hoverPhoto: PhotoLibraryPosition | null
    readonly selection: SelectionState | null
}

export interface DisplayState {
    /** The target row height of the grid. The grid won't hit this value exactly, as it depends on the layout. */
    readonly gridRowHeight: number
}

/**
 * The position of a photo in the library view
 */
export interface PhotoLibraryPosition {
    sectionId: PhotoSectionId
    photoId: PhotoId
}

export interface SelectionState {
    readonly totalSelectedCount: number
    readonly sectionSelectionById: { [K in PhotoSectionId]?: SectionSelectionState }
}

export interface SectionSelectionState {
    readonly sectionId: PhotoSectionId
    readonly selectedCount: number
    readonly selectedPhotosById: 'all' | { [K in PhotoId]?: true }
}

export type PhotoCollection = Photo | Photo[] | PhotoLibraryPosition | SelectionState

export interface PreselectionRange {
    selected: boolean
    startSectionIndex: number
    startPhotoIndex: number
    endSectionIndex: number
    endPhotoIndex: number
}

export type SectionPreselection = 'all' | 'none' | { selected: boolean, startPhotoIndex: number, endPhotoIndex: number }


export type TagsState = {
    readonly ids: TagId[]
    readonly byId: TagById
}

/**
 * The top-level folders inside the photo roots (for the folder sidebar). Fetched once and refreshed
 * like the photo count - it does not change with the current library filter.
 */
export type FoldersState = {
    readonly ids: string[]
    readonly byId: { [name: string]: PhotoFolder }
}

export type SectionsState = {
    readonly fetchState: FetchState
    /** The total number of photos (when no filter is applied). Is null before fetched for the first time. */
    readonly totalPhotoCount: number | null
    /** The number of photos with the current filter applied */
    readonly photoCount: number
    readonly ids: PhotoSectionId[]
    readonly byId: PhotoSectionById
}


export type DetailState = {
    readonly currentPhoto: {
        readonly fetchState: FetchState
        readonly sectionId: PhotoSectionId
        readonly photoIndex: number
        readonly photoId: PhotoId
        /** Is `null` while loading */
        readonly photoWork: PhotoWork | null
    }
} | null


export interface InfoState {
    readonly showInLibrary: boolean
    readonly showInDetail: boolean
    /** Whether the colour grading panel is shown in the detail view */
    readonly showGradingInDetail: boolean
    readonly photoData?: InfoPhotoData
}

export enum InfoPhotoDataState { Loaded, Loading, MasterIsMissing, Error }
export interface LoadedInfoPhotoDataWithoutSection {
    state: InfoPhotoDataState.Loaded
    photoId: PhotoId
    photoDetail: PhotoDetail
    masterFileSize: number
    metaData: MetaData
    exifData: ExifData | null
}

export type InfoPhotoDataWithoutSection =
    LoadedInfoPhotoDataWithoutSection |
    {
        state: InfoPhotoDataState.MasterIsMissing
        photoId: PhotoId
        photoDetail: PhotoDetail
    } |
    {
        state: InfoPhotoDataState.Loading | InfoPhotoDataState.Error
        photoId: PhotoId
    }
export type LoadedInfoPhotoData = LoadedInfoPhotoDataWithoutSection & {
    sectionId: PhotoSectionId
}
export type InfoPhotoData = InfoPhotoDataWithoutSection & {
    sectionId: PhotoSectionId
}

export type ImportState = {
    readonly progress: ImportProgress
} | null


export type ExportState = {
    readonly photos: PhotoCollection
    readonly exportOptions: PhotoExportOptions
    readonly showRemoveInfoDesc: boolean
    readonly progress: PhotoExportProgress | null
} | null
