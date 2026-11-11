import React from 'react'
import { Props as SvgIconProps } from './SvgIcon'
import icon from '../../../../image/app-icon.png'
import { isSize } from 'app/util/GeometryTypes'

export const logoAspect = 1

export default class Logo extends React.Component<SvgIconProps> {
    render() {
        const { width, height, className, style, size = '1em' } = this.props
        return <img src={icon} width={width || (isSize(size) ? size.width : size)}
            height={height || (isSize(size) ? size.height : size)} className={className} style={style} alt='PhotoMaster'/>
    }
}
