package main

import (
	"net/http"
	"reflect"

	"github.com/flier268/ConvertZZ/ConvertZZ.Service/textConverter"
	"github.com/gin-gonic/gin"
)

var c *textConverter.Converter

func textConverterInit() {
	c = textConverter.New()
}

func v1TextConverterHelp(context *gin.Context) {
	temp := []string{}
	for _, option := range reflect.VisibleFields(reflect.TypeOf(textConverter.Converter{})) {
		temp = append(temp, option.Name)
	}
	context.String(http.StatusOK,
		`
# Convert
This api can convert chinese, there are serveral mode.
mode list: %v
api/vi/convert/:mode/*text
Example:
    localhost/api/vi/convert/s2t/简体中文转繁体中文
    localhost/api/vi/convert/t2s/繁體中文轉簡體中文

# Segmente
mode list: %v
api/vi/seg/:mode/*text
Example:
    localhost/api/vi/seg/s2t/简体中文转繁体中文
    localhost/api/vi/seg/t2s/繁體中文轉簡體中文

# Just convert
mode list: %v
api/vi/justConvert/:mode/*text
Example:
    localhost/api/vi/justConvert/s2t/简体中文转繁体中文
    localhost/api/vi/justConvert/t2s/繁體中文轉簡體中文
        `, temp, temp, temp)
}

func v1TextConvertConvert(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output := c.Convert(text, option)
	wrapResponse(context, output, nil)
}

func v1TextConvertSeg(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output := c.Seg(text, option)
	wrapResponse(context, output, nil)
}

func v1TextConvertJustConvert(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output, err := c.JustConvert(text, option)
	wrapResponse(context, output, err)
}
