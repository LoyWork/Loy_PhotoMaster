import classNames from 'classnames'
import React from 'react'
import { connect } from 'react-redux'
import { FaRegStar, FaTrashAlt, FaImages, FaTag, FaFolder, FaChevronRight, FaChevronDown, FaClone, FaCopy } from 'react-icons/fa'

import { TagId, TagById, PhotoFolder } from 'app/CommonTypes'
import { msg } from 'app/i18n/i18n'
import { setLibraryFilter } from 'app/controller/PhotoController'
import { AppState } from 'app/state/StateTypes'
import { showFolderContextMenu } from 'app/ui/widget/FolderContextMenu'

import './FolderSidebar.less'


type SimpleFilterType = 'all' | 'favorites' | 'trash'

const iconBySimpleFilter: { [K in SimpleFilterType]: JSX.Element } = {
    all: <FaImages/>,
    favorites: <FaRegStar/>,
    trash: <FaTrashAlt/>,
}

interface SidebarEntry {
    key: string
    icon: JSX.Element
    label: string
    count?: number
    isSelected: boolean
    onSelect(): void
}

interface OwnProps {
    className?: any
    cleanupView?: 'similar' | 'duplicates'
    onOpenSimilar?(): void
    onOpenDuplicates?(): void
    onNavigate?(): void
}

interface StateProps {
    folderIds: string[]
    folderById: { [name: string]: PhotoFolder }
    libraryFilter: AppState['library']['filter']
    tagIds: TagId[]
    tagById: TagById
}

interface DispatchProps {
    setLibraryFilter(newFilter: any): void
}

export interface Props extends OwnProps, StateProps, DispatchProps {
}

/**
 * The left sidebar: the single place to choose what the library shows - the photo view (all /
 * favorites / trash, formerly the filter button's menu), the tags and the top-level folders inside
 * the photo directories.
 */
export class FolderSidebar extends React.Component<Props, { collapsed: Set<string> }> {

    state = { collapsed: new Set<string>() }

    private renderFolderTree(): JSX.Element[] {
        const { folderIds, folderById, libraryFilter } = this.props
        const children = new Map<string | null, string[]>()
        for (const id of folderIds) {
            const folder = folderById[id]
            if (!folder || folder.count <= 0) continue
            const parent = folder.parentPath && folderById[folder.parentPath] ? folder.parentPath : null
            children.set(parent, [...(children.get(parent) || []), id])
        }
        const renderBranch = (parent: string | null, depth: number): JSX.Element[] =>
            (children.get(parent) || []).flatMap(id => {
                const folder = folderById[id]
                const hasChildren = !!children.get(id)?.length
                const expanded = !this.state.collapsed.has(id)
                return [
                    <div key={id} className={classNames('FolderSidebar-item FolderSidebar-folderRow', {
                        isSelected: !this.props.cleanupView && libraryFilter.filterType === 'folder' && libraryFilter.folder === id,
                    })} style={{ paddingLeft: 8 + depth * 14 }}
                        onContextMenu={event => {
                            if (folder.path) showFolderContextMenu(folder.path, event)
                            else event.preventDefault()
                        }}>
                        {hasChildren ? <button className='FolderSidebar-expand' aria-expanded={expanded}
                            aria-label={folder.name} onClick={() => this.setState(state => {
                                const collapsed = new Set(state.collapsed)
                                if (collapsed.has(id)) collapsed.delete(id)
                                else collapsed.add(id)
                                return { collapsed }
                            })}>{expanded ? <FaChevronDown/> : <FaChevronRight/>}</button> :
                            <span className='FolderSidebar-expandPlaceholder'/>}
                        <button className='FolderSidebar-folderSelect' title={folder.path || folder.name}
                            aria-current={!this.props.cleanupView && libraryFilter.filterType === 'folder' && libraryFilter.folder === id ? 'true' : undefined}
                            onClick={() => this.onSelectFolder(id)}>
                            <span className='FolderSidebar-itemIcon'><FaFolder/></span>
                            <span className='FolderSidebar-itemName'>{folder.name}</span>
                            <span className='FolderSidebar-itemCount'>{folder.count}</span>
                        </button>
                    </div>,
                    ...(hasChildren && expanded ? renderBranch(id, depth + 1) : []),
                ]
            })
        return renderBranch(null, 0)
    }

    private onSelectSimple(filterType: SimpleFilterType) {
        this.props.onNavigate?.()
        this.props.setLibraryFilter({ filterType })
    }

    private onSelectTag(tagId: TagId) {
        this.props.onNavigate?.()
        this.props.setLibraryFilter({ filterType: 'tag', tagId })
    }

    private onSelectFolder(folder: string) {
        this.props.onNavigate?.()
        this.props.setLibraryFilter({ filterType: 'folder', folder })
    }

    private renderSectionTitle(title: string): JSX.Element {
        return <div className='FolderSidebar-sectionTitle'>{title}</div>
    }

    private renderEntries(entries: SidebarEntry[]): JSX.Element[] {
        const { props } = this
        return entries.map(entry =>
            <button type='button'
                key={entry.key}
                className={classNames('FolderSidebar-item FolderSidebar-navButton', { isSelected: entry.isSelected && !props.cleanupView })}
                aria-current={entry.isSelected && !props.cleanupView ? 'page' : undefined}
                title={entry.label}
                onClick={entry.onSelect}
            >
                <span className='FolderSidebar-itemIcon'>{entry.icon}</span>
                <span className='FolderSidebar-itemName'>{entry.label}</span>
                {(entry.count !== undefined) &&
                    <span className='FolderSidebar-itemCount'>{entry.count}</span>
                }
            </button>
        )
    }

    render() {
        const { props } = this
        const { libraryFilter } = props

        const photoEntries: SidebarEntry[] = [
            {
                key: 'all',
                icon: iconBySimpleFilter.all,
                label: msg('LibraryFilterButton_filter_all'),
                isSelected: libraryFilter.filterType === 'all',
                onSelect: () => this.onSelectSimple('all'),
            },
            {
                key: 'favorites',
                icon: iconBySimpleFilter.favorites,
                label: msg('LibraryFilterButton_filter_favorites'),
                isSelected: libraryFilter.filterType === 'favorites',
                onSelect: () => this.onSelectSimple('favorites'),
            },
            {
                key: 'trash',
                icon: iconBySimpleFilter.trash,
                label: msg('LibraryFilterButton_filter_trash'),
                isSelected: libraryFilter.filterType === 'trash',
                onSelect: () => this.onSelectSimple('trash'),
            },
        ]

        return (
            <div className={classNames(props.className, 'FolderSidebar')} onContextMenu={event => event.preventDefault()}>
                <div className='FolderSidebar-body'>
                    {this.renderEntries(photoEntries)}
                    {props.onOpenSimilar && <button type='button'
                        className={classNames('FolderSidebar-item FolderSidebar-navButton', { isSelected: props.cleanupView === 'similar' })}
                        aria-current={props.cleanupView === 'similar' ? 'page' : undefined}
                        onClick={props.onOpenSimilar}>
                        <span className='FolderSidebar-itemIcon'><FaClone/></span>
                        <span className='FolderSidebar-itemName'>{msg('Similar_title')}</span>
                    </button>}
                    {props.onOpenDuplicates && <button type='button'
                        className={classNames('FolderSidebar-item FolderSidebar-navButton', { isSelected: props.cleanupView === 'duplicates' })}
                        aria-current={props.cleanupView === 'duplicates' ? 'page' : undefined}
                        onClick={props.onOpenDuplicates}>
                        <span className='FolderSidebar-itemIcon'><FaCopy/></span>
                        <span className='FolderSidebar-itemName'>{msg('Duplicates_title')}</span>
                    </button>}
                    {!!props.tagIds.length &&
                        <>
                            {this.renderSectionTitle(msg('LibraryFilterButton_tag_title'))}
                            {this.renderEntries(props.tagIds.map(tagId => {
                                const tag = props.tagById[tagId]
                                return {
                                    key: `tag-${tagId}`,
                                    icon: <FaTag/>,
                                    label: tag.title,
                                    isSelected: libraryFilter.filterType === 'tag' && libraryFilter.tagId === tagId,
                                    onSelect: () => this.onSelectTag(tagId),
                                }
                            }))}
                        </>
                    }
                    {!!props.folderIds.length &&
                        <>
                            {this.renderSectionTitle(msg('FolderSidebar_title'))}
                            {this.renderFolderTree()}
                        </>
                    }
                </div>
            </div>
        )
    }

}


const Connected = connect<StateProps, DispatchProps, OwnProps, AppState>(
    (state: AppState, props: OwnProps) => {
        const { folders } = state.data
        return {
            ...props,
            folderIds: folders.ids,
            folderById: folders.byId,
            libraryFilter: state.library.filter,
            tagIds: state.data.tags.ids,
            tagById: state.data.tags.byId,
        }
    },
    dispatch => ({
        setLibraryFilter,
    })
)(FolderSidebar)

export default Connected
