import React, { ReactNode } from 'react'
import { bindActionCreators } from 'redux'
import { connect } from 'react-redux'
import { Button, NonIdealState } from '@blueprintjs/core'
import classNames from 'classnames'
import { convertFileSrc } from '@tauri-apps/api/core'

import BackgroundClient from 'app/BackgroundClient'
import { ImportProgress } from 'app/CommonTypes'
import { createGlobalCommands } from 'app/controller/GlobalCommandController'
import { addCommandGroup, Command, CommandGroupId, removeCommandGroup } from 'app/controller/HotkeyController'
import { defaultLibrarySelectionController } from 'app/controller/LibrarySelectionController'
import { msg } from 'app/i18n/i18n'
import PhotoDetailPane from 'app/ui/detail/PhotoDetailPane'
import ExportDialog from 'app/ui/export/ExportDialog'
import Library from 'app/ui/library/Library'
import SelectionSummary from 'app/ui/library/SelectionSummary'
import ImportProgressButton from 'app/ui/ImportProgressButton'
import SettingsPane from 'app/ui/SettingsPane'
import AppUpdateDialog from 'app/ui/AppUpdateDialog'
import ErrorBoundary from 'app/ui/widget/ErrorBoundary'
import { openSettingsAction, setShowSidebarAction } from 'app/state/actions'
import store from 'app/state/store'
import { AppState, MainViewState } from 'app/state/StateTypes'

import WindowControls from './WindowControls'

import { defaultBlurStrength } from 'app/style/variables'

import './App.less'


interface OwnProps {
    style?: any
    className?: any
}

interface StateProps {
    isFullScreen: boolean
    hasWebGLSupport: boolean
    hasNativeTrafficLightButtons: boolean
    showWindowsButtons: boolean
    globalCommands: Command[] | null
    selectedCount: number
    mainView: MainViewState
    importProgress: ImportProgress | null
    showExport: boolean
    showSidebar: boolean
    hasSidebarContent: boolean
    wallpaperPath: string | null
    blurStrength: number
}

interface DispatchProps {
    toggleFullScreen(): void
    clearSelection(): void
    openSettings(): void
    toggleImportPaused(): void
    cancelImport(): void
    toggleSidebar(): void
}

interface Props extends OwnProps, StateProps, DispatchProps {
}

class App extends React.Component<Props> {

    private commandGroupId: CommandGroupId | null = null

    componentDidMount() {
        if (this.props.globalCommands) {
            this.commandGroupId = addCommandGroup(this.props.globalCommands)
        }
        this.applyAppearance()
    }

    componentDidUpdate(prevProps: Props) {
        if (prevProps.wallpaperPath !== this.props.wallpaperPath ||
            prevProps.blurStrength !== this.props.blurStrength)
        {
            this.applyAppearance()
        }
    }

    /** Writes the appearance settings to CSS variables on the app root. */
    private applyAppearance() {
        const rootStyle = document.documentElement.style
        rootStyle.setProperty('--glass-blur-px', `${this.props.blurStrength}px`)
    }

    componentWillUnmount() {
        if (this.commandGroupId) {
            removeCommandGroup(this.commandGroupId)
        }
    }

    render() {
        const { props } = this

        let modalView: ReactNode | null = null
        if (props.showExport) {
            modalView = <ExportDialog />
        }

        let mainView: ReactNode | null = null
        if (!props.hasWebGLSupport) {
            mainView = (
                <div className='App-mainView App-globalError'>
                    <NonIdealState
                        icon='media'
                        title={msg('App_error_noWebGL_title')}
                        description={msg('App_error_noWebGL_desc')}
                    />
                </div>
            )
        } else if (props.mainView === 'settings') {
            mainView = <SettingsPane className='App-mainView'/>
        } else if (props.mainView === 'detail') {
            mainView = <PhotoDetailPane className='App-mainView' isActive={!modalView} />
        }

        const libraryTopBarLeftItem = (
            <>
                {props.selectedCount === 0 && props.isFullScreen && <Button minimal icon='minimize' onClick={props.toggleFullScreen}/>}
                <Button minimal icon='drawer-left' active={props.showSidebar}
                    disabled={!props.hasSidebarContent}
                    title={msg('App_toggleSidebar')} aria-label={msg('App_toggleSidebar')}
                    aria-pressed={props.showSidebar} onClick={props.toggleSidebar}/>
                {props.selectedCount === 0 ? <>
                    <Button minimal icon='cog' onClick={props.openSettings}/>
                </> : <SelectionSummary selectedCount={props.selectedCount} onClearSelection={props.clearSelection}/>}
            </>
        )

        return (
            <ErrorBoundary className='bp3-dark'>
                <div className={classNames('App', { hasNativeTrafficLightButtons: props.hasNativeTrafficLightButtons, hasWindowsButtons: props.showWindowsButtons })}>
                    {props.wallpaperPath
                        ? <div
                            className='App-wallpaper App-wallpaper-image'
                            style={{ backgroundImage: `url("${convertFileSrc(props.wallpaperPath)}")` }}
                        />
                        : <div className='App-wallpaper'/>
                    }
                    <Library
                        className='App-container'
                        style={mainView ? { display: 'none' } : undefined}
                        topBarLeftItem={libraryTopBarLeftItem}
                        bottomBarLeftItem={props.importProgress &&
                            <ImportProgressButton
                                progress={props.importProgress}
                                toggleImportPaused={props.toggleImportPaused}
                                cancelImport={props.cancelImport}
                            />
                        }
                        isActive={!mainView && !modalView}
                    />
                    {mainView}
                    {modalView}
                    <AppUpdateDialog/>
                    {props.showWindowsButtons &&
                        <WindowControls
                            className='App-windowControls'
                        />
                    }
                </div>
            </ErrorBoundary>
        )
    }
}


const Connected = connect<StateProps, DispatchProps, OwnProps, AppState>(
    (state, props) => {
        const { uiConfig } = state.data
        return {
            ...props,
            isFullScreen: state.navigation.isFullScreen,
            hasWebGLSupport: state.navigation.hasWebGLSupport,
            hasNativeTrafficLightButtons: uiConfig.windowStyle === 'nativeTrafficLight' && !state.navigation.isFullScreen,
            showWindowsButtons: uiConfig.windowStyle === 'windowsButtons',
            globalCommands: uiConfig.hasNativeMenu ? null : createGlobalCommands(),
            selectedCount: state.library.selection?.totalSelectedCount ?? 0,
            mainView: state.navigation.mainView,
            importProgress: state.import && state.import.progress,
            showExport: !!state.export,
            showSidebar: state.navigation.showSidebar,
            hasSidebarContent: true,
            wallpaperPath: state.data.settings.appearance?.wallpaperPath ?? null,
            blurStrength: state.data.settings.appearance?.blurStrength ?? defaultBlurStrength,
        }
    },
    dispatch => ({
        toggleFullScreen() { BackgroundClient.toggleFullScreen() },
        clearSelection() { defaultLibrarySelectionController.clearSelection() },
        toggleImportPaused() { BackgroundClient.toggleImportPaused() },
        cancelImport() { BackgroundClient.cancelImport() },
        toggleSidebar() { dispatch(setShowSidebarAction(!store.getState().navigation.showSidebar)) },
        ...bindActionCreators({
            openSettings: openSettingsAction
        }, dispatch)
    })
)(App)

export default Connected
