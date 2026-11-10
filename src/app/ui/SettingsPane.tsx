import React, { ReactNode } from 'react'
import { connect } from 'react-redux'
import { Button, Slider } from '@blueprintjs/core'
import classnames from 'classnames'
import { convertFileSrc } from '@tauri-apps/api/core'
import { FaChevronLeft } from 'react-icons/fa'

import BackgroundClient from 'app/BackgroundClient'
import { AppearanceSettings, Settings } from 'app/CommonTypes'
import { showError } from 'app/ErrorPresenter'
import { msg } from 'app/i18n/i18n'
import LicenseDialog from 'app/ui/license/LicenseDialog'
import Toolbar from 'app/ui/widget/Toolbar'
import List from 'app/ui/widget/List'
import LogoDecoration from 'app/ui/widget/LogoDecoration'
import { setSettingsAction, closeSettingsAction } from 'app/state/actions'
import { AppState } from 'app/state/StateTypes'
import { bindMany } from 'app/util/LangUtil'
import { formatNumber } from 'app/util/TextUtil'

import './SettingsPane.less'


const minBlurStrength = 0
const maxBlurStrength = 60
const defaultBlurStrength = 28


export interface OwnProps {
    style?: any
    className?: any
}

interface StateProps {
    settings: Settings
    version: string
}

interface DispatchProps {
    selectDirectories(): Promise<string[] | undefined>
    selectWallpaper(): Promise<string | undefined>
    onSettingsChange(settings: Settings): void
    onClose(settings: Settings, startImport: boolean): void
}

export interface Props extends OwnProps, StateProps, DispatchProps {}

interface State {
    showLicenses: boolean
}

export class SettingsPane extends React.Component<Props, State> {

    constructor(props: Props) {
        super(props)
        this.state = {
            showLicenses: false,
        }
        bindMany(this, 'onPhotoDirsChange', 'onAddPhotoDir', 'onClose', 'onCloseAndImport', 'getDecorationWidth',
            'onShowLicenses', 'onLicensesClosed', 'onChooseWallpaper', 'onResetWallpaper')
    }

    private onPhotoDirsChange(photoDirs: string[]) {
        this.props.onSettingsChange({ ...this.props.settings, photoDirs })
    }

    private onAddPhotoDir() {
        const { props } = this
        props.selectDirectories()
            .then(dirs => {
                if (dirs) {
                    const nextPhotoDirs = [ ...props.settings.photoDirs, ...dirs ]
                    nextPhotoDirs.sort()
                    props.onSettingsChange({ ...props.settings, photoDirs: nextPhotoDirs })
                }
            })
            .catch(error => {
                console.error('Selecting dirs failed', error)
            })
    }

    private onClose() {
        this.props.onClose(this.props.settings, false)
    }

    private onCloseAndImport() {
        this.props.onClose(this.props.settings, true)
    }

    private onShowLicenses() {
        this.setState({ showLicenses: true })
    }

    private onLicensesClosed() {
        this.setState({ showLicenses: false })
    }

    private get appearance(): AppearanceSettings {
        return this.props.settings.appearance ?? {}
    }

    private changeAppearance(change: Partial<AppearanceSettings>) {
        const next: AppearanceSettings = { ...this.appearance, ...change }
        // An empty appearance object carries no information — drop it, so the settings file stays clean
        const hasContent = next.wallpaperPath !== undefined || next.blurStrength !== undefined
        this.props.onSettingsChange({ ...this.props.settings, appearance: hasContent ? next : undefined })
    }

    private onChooseWallpaper() {
        this.props.selectWallpaper()
            .then(path => {
                if (path) {
                    this.changeAppearance({ wallpaperPath: path })
                }
            })
            .catch(error => {
                console.error('Selecting the wallpaper failed', error)
            })
    }

    private onResetWallpaper() {
        this.changeAppearance({ wallpaperPath: undefined })
    }

    private onBlurStrengthChange(value: number) {
        this.changeAppearance({ blurStrength: (value === defaultBlurStrength) ? undefined : value })
    }

    private getDecorationWidth(containerWidth: number): number {
        return containerWidth - 800
    }

    render() {
        const { props, state } = this
        const { settings } = props
        return (
            <div className={classnames(props.className, 'SettingsPane')} style={props.style}>
                <LogoDecoration getDecorationWidth={this.getDecorationWidth}/>
                <Toolbar
                    className="SettingsPane-topBar"
                    isTopBar={true}
                    isLeft={true}
                >
                    <Button onClick={this.onClose}>
                        <FaChevronLeft/>
                        <span>{msg('common_backToLibrary')}</span>
                    </Button>
                    <Toolbar.Spacer isTopBar/>
                </Toolbar>                
                <div className='SettingsPane-body'>
                    <div className='SettingsPane-content'>
                        <h1>{msg('Settings_title')}</h1>
                        <section className='SettingsPane-section'>
                            <h2>{msg('Settings_scanDirsTitle')}</h2>
                            {settings.photoDirs.length === 0 &&
                                <p>{msg('Settings_selectPhotoDirs')}</p>
                            }
                            {settings.photoDirs.length > 0 &&
                                <>
                                    <p>{msg('Settings_photoDirs')}</p>
                                    <List
                                        className='SettingsPane-photoDirs'
                                        items={settings.photoDirs}
                                        renderItem={renderPhotoDir}
                                        onItemsChange={this.onPhotoDirsChange}
                                    />
                                </>
                            }
                            <Button
                                className='SettingsPane-addPhotoDir'
                                icon='folder-new'
                                text={msg('Settings_addPhotoDir')}
                                onClick={this.onAddPhotoDir}
                            />
                            <Button
                                className='SettingsPane-startScan'
                                large={true}
                                intent='primary'
                                icon='refresh'
                                text={msg('Settings_startScan')}
                                onClick={this.onCloseAndImport}
                            />
                        </section>
                        <section className='SettingsPane-section'>
                            <h2>{msg('Settings_appearanceTitle')}</h2>
                            <p className='SettingsPane-wallpaperLabel'>{msg('Settings_wallpaperLabel')}</p>
                            <div className='SettingsPane-wallpaperPreview'
                                style={this.appearance.wallpaperPath
                                    ? { backgroundImage: `url("${convertFileSrc(this.appearance.wallpaperPath)}")` }
                                    : undefined}>
                                {!this.appearance.wallpaperPath &&
                                    <span className='SettingsPane-wallpaperPreviewDefault'>Picturama</span>
                                }
                            </div>
                            <div className='SettingsPane-wallpaperActions'>
                                <Button
                                    icon='media'
                                    text={msg('Settings_wallpaperChoose')}
                                    onClick={this.onChooseWallpaper}
                                />
                                {this.appearance.wallpaperPath !== undefined &&
                                    <Button
                                        icon='undo'
                                        text={msg('Settings_wallpaperReset')}
                                        onClick={this.onResetWallpaper}
                                    />
                                }
                            </div>
                            <div className='SettingsPane-blurSlider'>
                                <div className='SettingsPane-blurLabel'>
                                    {msg('Settings_blurLabel',
                                        formatNumber(this.appearance.blurStrength ?? defaultBlurStrength))}
                                </div>
                                <Slider
                                    value={this.appearance.blurStrength ?? defaultBlurStrength}
                                    min={minBlurStrength}
                                    max={maxBlurStrength}
                                    stepSize={1}
                                    labelRenderer={false}
                                    onChange={value => this.onBlurStrengthChange(value)}
                                />
                            </div>
                        </section>
                        <div className='SettingsPane-about'>
                            <span>Picturama</span>
                            <span className='SettingsPane-appVersion'>{props.version}</span>
                            <Button
                                small
                                text={msg('common_licenses')}
                                onClick={this.onShowLicenses}
                            />
                        </div>
                    </div>
                </div>
                {state.showLicenses &&
                    <LicenseDialog onClosed={this.onLicensesClosed}/>
                }
            </div>
        )
    }

}


function renderPhotoDir(photoDir: string): ReactNode {
    return photoDir
}

const Connected = connect<StateProps, DispatchProps, OwnProps, AppState>(
    (state: AppState, props: OwnProps) => {
        return {
            ...props,
            settings: state.data.settings,
            version: state.data.uiConfig.version,
        }
    },
    dispatch => ({
        selectDirectories: BackgroundClient.selectScanDirectories,
        selectWallpaper: BackgroundClient.selectWallpaperFile,
        onSettingsChange(settings: Settings) {
            dispatch(setSettingsAction(settings))
        },
        onClose(settings: Settings, startImport: boolean) {
            dispatch(closeSettingsAction())
            BackgroundClient.storeSettings(settings)
                .then(() => {
                    if (startImport) {
                        BackgroundClient.startImport()
                    }
                })
                .catch(error => {
                    showError('Applying new settings failed', error)
                })
        },
    })
)(SettingsPane)

export default Connected
