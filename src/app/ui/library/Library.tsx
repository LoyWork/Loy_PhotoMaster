import classNames from 'classnames'
import React from 'react'
import { connect } from 'react-redux'
import { bindActionCreators } from 'redux'
import { Button, NonIdealState, Spinner, MaybeElement, Icon, INonIdealStateProps } from '@blueprintjs/core'

import BackgroundClient from 'app/BackgroundClient'
import { PhotoId, Photo, PhotoSectionId, PhotoSectionById, PhotoFilterType, EmptyTrashResult } from 'app/CommonTypes'
import { setDetailPhotoById } from 'app/controller/DetailController'
import { defaultLibrarySelectionController, LibrarySelectionController } from 'app/controller/LibrarySelectionController'
import { GetGridLayoutFunction, getGridLayoutAndUpdateStore, createThumbnail } from 'app/controller/LibraryController'
import { PhotoActionController, defaultPhotoActionController } from 'app/controller/PhotoActionController'
import { fetchTotalPhotoCount, fetchSections, fetchPhotoFolders, getThumbnailSrc, onPhotosTrashed } from 'app/controller/PhotoController'
import { fetchTags, setPhotoTags } from 'app/controller/PhotoTagController'
import { showError } from 'app/ErrorPresenter'
import { msg } from 'app/i18n/i18n'
import { setGridRowHeightAction, setShowInfoAction, setShowGradingAction, setShowSidebarAction } from 'app/state/actions'
import store from 'app/state/store'
import { getInfoPhoto, getPreselectionRange, getTagTitles } from 'app/state/selectors'
import { AppState, InfoPhotoData, PhotoLibraryPosition, PreselectionRange, SelectionState } from 'app/state/StateTypes'
import PhotoInfo from 'app/ui/info/PhotoInfo'
import { keySymbols } from 'app/UiConstants'
import { FetchState } from 'app/UITypes'
import CancelablePromise from 'app/util/CancelablePromise'
import { bindMany } from 'app/util/LangUtil'
import { getCollectionSize, getFirstPhotoOfCollection } from 'app/util/PhotoCollectionResolver'
import PhotoToolsToolbar from 'app/ui/widget/PhotoToolsToolbar'

import LibraryTopBar from './LibraryTopBar'
import LibraryBottomBar from './LibraryBottomBar'
import FolderSidebar from './FolderSidebar'
import FolderSidebarResizer from './FolderSidebarResizer'
import Grid from './Grid'
import SimilarPhotos from './SimilarPhotos'
import Toolbar from 'app/ui/widget/Toolbar'

import './Library.less'


interface OwnProps {
    style?: any
    className?: any
    topBarLeftItem?: MaybeElement
    bottomBarLeftItem?: MaybeElement
    isActive: boolean
}

interface StateProps {
    isMac: boolean
    hasPhotoDirs: boolean
    isFetching: boolean
    isImporting: boolean
    libraryFilterType: PhotoFilterType
    photoCount: number
    totalPhotoCount: number | null
    sectionIds: PhotoSectionId[]
    sectionById: PhotoSectionById
    activePhoto: PhotoLibraryPosition | null
    selection: SelectionState | null
    preselectionRange: PreselectionRange | null
    showInfo: boolean
    hasFolders: boolean
    showSidebar: boolean
    hasSidebarContent: boolean
    toggleSidebar: () => void
    infoPhoto?: Photo
    infoPhotoData?: InfoPhotoData
    tags: string[]
    gridRowHeight: number
    photoActionController: PhotoActionController
    librarySelectionController: LibrarySelectionController
    onPhotosTrashed(result: EmptyTrashResult): void
}

interface DispatchProps {
    fetchTotalPhotoCount: () => void
    fetchPhotoFolders: () => void
    fetchSections: () => void
    fetchTags(): void
    getGridLayout: GetGridLayoutFunction
    getThumbnailSrc: (photo: Photo) => string
    createThumbnail: (sectionId: PhotoSectionId, photo: Photo) => CancelablePromise<string>
    setGridRowHeight: (gridRowHeight: number) => void
    setDetailPhotoById: (sectionId: PhotoSectionId, photoId: PhotoId) => void
    setPhotoTags: (photo: Photo, tags: string[]) => void
    setShowInfo(showInfo: boolean): void
    startScanning: () => void
}

export interface Props extends OwnProps, StateProps, DispatchProps {
}

export class Library extends React.Component<Props, { cleanupView: 'similar' | 'duplicates' | null }> {

    state: { cleanupView: 'similar' | 'duplicates' | null } = { cleanupView: null }

    constructor(props: Props) {
        super(props)
        bindMany(this, 'toggleShowInfo', 'openGrading')
    }

    componentDidMount() {
        const { props } = this
        props.fetchTotalPhotoCount()
        props.fetchSections()
        props.fetchPhotoFolders()
        props.fetchTags()
    }

    private toggleShowInfo() {
        const { props } = this
        props.setShowInfo(!props.showInfo)
    }

    private openGrading() {
        const first = getFirstPhotoOfCollection(this.props.selection || this.props.activePhoto)
        if (first) {
            this.props.setDetailPhotoById(first.sectionId, first.photo.id)
            store.dispatch(setShowGradingAction(true))
        }
    }

    render() {
        const { props } = this
        if (this.state.cleanupView) {
            return <div className={classNames(props.className, 'Library bp3-dark hasSimilar', {
                hasSidebar: props.showSidebar && props.hasSidebarContent,
            })} style={props.style}>
                <FolderSidebar className='Library-folderSidebar' cleanupView={this.state.cleanupView}
                    onOpenSimilar={() => this.setState({ cleanupView: 'similar' })}
                    onOpenDuplicates={() => this.setState({ cleanupView: 'duplicates' })}
                    onNavigate={() => this.setState({ cleanupView: null })}/>
                <FolderSidebarResizer showInfo={false}/>
                <Toolbar className='Library-topBar' isTopBar isLeft isRight>
                    {props.topBarLeftItem}
                    <Toolbar.Spacer isTopBar/>
                </Toolbar>
                <div className='Library-body'>
                    <SimilarPhotos key={this.state.cleanupView} kind={this.state.cleanupView}/>
                </div>
            </div>
        }
        const ctrlOrMacCommand = props.isMac ? keySymbols.macCommand : keySymbols.ctrl

        let nonIdealStateProps: INonIdealStateProps | null = null
        if (props.totalPhotoCount === 0 && !props.isFetching && !props.isImporting) {
            if (!props.hasPhotoDirs) {
                const descriptionSplits = msg('Library_noSettings_message').split('{0}')
                nonIdealStateProps = {
                    icon: 'zoom-out',
                    title: msg('Library_noPhotos_title'),
                    description: (
                        <>
                            {descriptionSplits[0]}
                            <Icon icon='cog' style={{ verticalAlign: 'middle' }}/>
                            {descriptionSplits[1]}
                        </>
                    )
                }
            } else {
                const descriptionSplits = msg('Library_noPhotos_message').split('{0}')
                nonIdealStateProps = {
                    icon: 'zoom-out',
                    title: msg('Library_noPhotos_title'),
                    description: (
                        <>
                            {descriptionSplits[0]}
                            <code>{ctrlOrMacCommand}</code>+<code>R</code>
                            {descriptionSplits[1]}
                        </>
                    ),
                    action: (
                        <div className="bp3-dark">
                            <Button onClick={props.startScanning}>{msg('Library_startScanning')}</Button>
                        </div>
                    )
                }
            }
        } else if (props.photoCount === 0 && !props.isFetching && !props.isImporting) {
            let title: string
            switch (props.libraryFilterType) {
                case 'favorites': title = msg('Library_emptyFavorites'); break
                case 'trash':     title = msg('Library_emptyTrash'); break
                default:          title = msg('Library_emptyView'); break
            }

            nonIdealStateProps = {
                icon: props.libraryFilterType === 'trash' ? 'tick' : 'zoom-out',
                title,
                description: msg('Library_selectOtherView')
            }
        }

        const showGrid = !nonIdealStateProps

        return (
            <div
                ref="library"
                className={classNames(props.className, 'Library', { 'bp3-dark': showGrid, hasGrid: showGrid, hasRightSidebar: props.showInfo, hasSidebar: props.showSidebar && props.hasSidebarContent })}
                style={props.style}
            >
                <FolderSidebar className="Library-folderSidebar" onOpenSimilar={() => {
                    props.librarySelectionController.clearSelection()
                    this.setState({ cleanupView: 'similar' })
                }} onOpenDuplicates={() => {
                    props.librarySelectionController.clearSelection()
                    this.setState({ cleanupView: 'duplicates' })
                }}/>
                <FolderSidebarResizer showInfo={props.showInfo}/>
                <LibraryTopBar
                    className="Library-topBar"
                    isActive={props.isActive}
                    leftItem={props.topBarLeftItem}
                    selectedPhotos={props.selection || props.activePhoto}
                    hasSelection={!!props.selection && getCollectionSize(props.selection) > 0}
                    isShowingTrash={props.libraryFilterType === 'trash'}
                    isShowingInfo={props.showInfo}
                    photosCount={props.photoCount}
                    photoActionController={props.photoActionController}
                    toggleShowInfo={this.toggleShowInfo}
                    toggleShowGrading={this.openGrading}
                    onPhotosTrashed={props.onPhotosTrashed}
                />
                <div className="Library-body">
                    {nonIdealStateProps &&
                        <NonIdealState {...nonIdealStateProps}/>
                    }
                    {showGrid &&
                        <Grid
                            className="Library-grid"
                            isActive={props.isActive}
                            sectionIds={props.sectionIds}
                            sectionById={props.sectionById}
                            activePhoto={props.activePhoto}
                            selection={props.selection}
                            preselectionRange={props.preselectionRange}
                            gridRowHeight={props.gridRowHeight}
                            librarySelectionController={props.librarySelectionController}
                            photoActionController={props.photoActionController}
                            isShowingTrash={props.libraryFilterType === 'trash'}
                            getGridLayout={props.getGridLayout}
                            getThumbnailSrc={props.getThumbnailSrc}
                            createThumbnail={props.createThumbnail}
                            setDetailPhotoById={props.setDetailPhotoById}
                        />
                    }
                    {(props.isImporting ? props.photoCount === 0 : props.isFetching) &&
                        <Spinner className="Library-spinner" size={Spinner.SIZE_LARGE} />
                    }
                </div>
                <LibraryBottomBar
                    className="Library-bottomBar"
                    leftItem={props.bottomBarLeftItem}
                    showSlider={props.photoCount > 0}
                    gridRowHeight={props.gridRowHeight}
                    setGridRowHeight={props.setGridRowHeight}
                />
                <PhotoInfo
                    className="Library-rightSidebar"
                    isActive={props.showInfo}
                    photo={props.infoPhoto}
                    photoData={props.infoPhotoData}
                    tags={props.tags}
                    closeInfo={this.toggleShowInfo}
                    setPhotoTags={props.setPhotoTags}
                />
                <PhotoToolsToolbar
                    showInfo={props.showInfo}
                    showGrading={false}
                    canGrade={getCollectionSize(props.selection || props.activePhoto) > 0}
                    disabled={!props.isActive}
                    toggleShowInfo={this.toggleShowInfo}
                    toggleShowGrading={this.openGrading}
                />
            </div>
        );
    }
}


const Connected = connect<StateProps, DispatchProps, OwnProps, AppState>(
    (state: AppState, props) => {
        const { sections } = state.data
        return {
            ...props,
            isMac: state.data.uiConfig.platform === 'macos',
            hasPhotoDirs: state.data.settings.photoDirs.length !== 0,
            isFetching: sections.totalPhotoCount === null || sections.fetchState === FetchState.FETCHING,
            isImporting: !!state.import && state.import.progress.phase !== 'error',
            libraryFilterType: state.library.filter.filterType,
            photoCount: sections.photoCount,
            totalPhotoCount: sections.totalPhotoCount,
            sectionIds: sections.ids,
            sectionById: sections.byId,
            activePhoto: state.library.activePhoto,
            selection: state.library.selection,
            preselectionRange: getPreselectionRange(state),
            showInfo: state.info.showInLibrary,
            hasFolders: state.data.folders.ids.length !== 0,
            showSidebar: state.navigation.showSidebar,
            hasSidebarContent: true,
            infoPhoto: getInfoPhoto(state),
            infoPhotoData: state.info.photoData,
            tags: getTagTitles(state),
            gridRowHeight: state.library.display.gridRowHeight,
            photoActionController: defaultPhotoActionController,
            librarySelectionController: defaultLibrarySelectionController,
            onPhotosTrashed,
        }
    },
    dispatch => ({
        fetchTotalPhotoCount,
        fetchPhotoFolders,
        fetchSections,
        fetchTags,
        getGridLayout: getGridLayoutAndUpdateStore,
        getThumbnailSrc,
        createThumbnail,
        setDetailPhotoById,
        setPhotoTags,
        toggleSidebar() {
            dispatch(setShowSidebarAction(!store.getState().navigation.showSidebar))
        },
        setShowInfo(showInfo: boolean) {
            dispatch(setShowInfoAction('library', showInfo))
        },
        startScanning: () => {
            BackgroundClient.startImport()
                .catch(error => showError('Starting import failed', error))
        },
        ...bindActionCreators({
            setGridRowHeight: setGridRowHeightAction,
        }, dispatch)
    })
)(Library)

export default Connected
