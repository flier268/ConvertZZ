package main

import (
	"encoding/json"
	"errors"
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

GET api/vi/textConverter/convert/:mode/*text
Example:
    GET localhost/api/vi/textConverter/convert/s2t/简体中文转繁体中文
    GET localhost/api/vi/textConverter/convert/t2s/繁體中文轉簡體中文

POST api/vi/textConverter/convert/:mode
Example:
    POST localhost/api/vi/textConverter/convert/s2t
	body: {
		"type": "string",
		"Context": "简体中文转繁体中文"
	}

	POST localhost/api/vi/textConverter/convert/s2t
	body: {
		"type": "[]string",
		"Context": ["简体中文转繁体中文", "简体中文转繁体中文2"]
	}

# Segmente
mode list: %v
GET api/vi/textConverter/seg/:mode/*text
Example:
    GET localhost/api/vi/textConverter/seg/s2t/简体中文转繁体中文
    GET localhost/api/vi/textConverter/seg/t2s/繁體中文轉簡體中文

POST api/vi/textConverter/seg/:mode
Example:
    POST localhost/api/vi/textConverter/seg/s2t
	body: {
		"type": "string",
		"Context": "简体中文转繁体中文"
	}

	POST localhost/api/vi/textConverter/seg/s2t
	body: {
		"type": "[]string",
		"Context": ["简体中文转繁体中文", "简体中文转繁体中文2"]
	}
# Just convert
mode list: %v
GET api/vi/textConverter/justConvert/:mode/*text
Example:
    GET localhost/api/vi/textConverter/justConvert/s2t/简体中文转繁体中文
    GET localhost/api/vi/textConverter/justConvert/t2s/繁體中文轉簡體中文

POST api/vi/textConverter/justConvert/:mode
Example:
    POST localhost/api/vi/textConverter/justConvert/s2t
	body: {
		"type": "string",
		"Context": "简体中文转繁体中文"
	}

	POST localhost/api/vi/textConverter/justConvert/s2t
	body: {
		"type": "[]string",
		"Context": ["简体中文转繁体中文", "简体中文转繁体中文2"]
	}	
        `, temp, temp, temp)
}

type v1BodyContext struct {
	Type    string          `json:"type"`
	Context json.RawMessage `json:"context"`
}

func v1CheckOptionAndJsonBody(context *gin.Context) (textConverter.Options, interface{}, error) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		return -1, nil, err
	}
	var bodyJSON v1BodyContext
	if err := context.ShouldBindJSON(&bodyJSON); err != nil {
		return option, nil, errors.New("Invalid body")
	}

	var parsedContext interface{}
	switch bodyJSON.Type {
	case "string":
		var str string
		if err := json.Unmarshal(bodyJSON.Context, &str); err != nil {
			return option, nil, err
		}
		parsedContext = str

	case "string[]":
	case "[]string":
		var strArr []string
		if err := json.Unmarshal(bodyJSON.Context, &strArr); err != nil {
			return option, nil, err
		}
		parsedContext = strArr

	default:
		return option, nil, errors.New("Unsupported type")
	}

	return option, parsedContext, nil
}

func v1PostTextConverterConvert(context *gin.Context) {
	option, bodyJSON, err := v1CheckOptionAndJsonBody(context)
	if err != nil {
		wrapResponse(context, "", err)
		return
	}

	switch v := bodyJSON.(type) {
	case string:
		// Handle single string
		str, err := c.Convert(v, option)
		if err != nil {
			wrapResponse(context, "", err)
			return
		}
		wrapResponse(context, str, nil)
	case []string:
		// Handle string array
		convertedTexts := make([]string, len(v))
		for index, s := range v {
			convertedTexts[index], err = c.Convert(s, option)
			if err != nil {
				wrapResponse(context, "", err)
				return
			}
		}
		wrapResponse(context, convertedTexts, nil)
	default:
		wrapResponse(context, "", errors.New("body type"))
	}
}

func v1PostTextConverterJustConvert(context *gin.Context) {
	option, bodyJSON, err := v1CheckOptionAndJsonBody(context)
	if err != nil {
		wrapResponse(context, "", err)
		return
	}

	switch v := bodyJSON.(type) {
	case string:
		// Handle single string
		str, err := c.JustConvert(v, option)
		if err != nil {
			wrapResponse(context, "", err)
			return
		}
		wrapResponse(context, str, nil)
	case []string:
		// Handle string array
		convertedTexts := make([]string, len(v))
		for index, s := range v {
			convertedTexts[index], err = c.JustConvert(s, option)
			if err != nil {
				wrapResponse(context, "", err)
				return
			}
		}
		wrapResponse(context, convertedTexts, nil)
	default:
		wrapResponse(context, "", errors.New("body type"))
	}
}
func v1PostTextConverterSeg(context *gin.Context) {
	option, bodyJSON, err := v1CheckOptionAndJsonBody(context)
	if err != nil {
		wrapResponse(context, "", err)
		return
	}

	switch v := bodyJSON.(type) {
	case string:
		// Handle single string
		str := c.Seg(v, option)
		wrapResponse(context, str, nil)
	case []string:
		// Handle string array
		convertedTexts := make([][]string, len(v))
		for index, s := range v {
			convertedTexts[index] = c.Seg(s, option)
		}
		wrapResponse(context, convertedTexts, nil)
	default:
		wrapResponse(context, "", errors.New("body type"))
	}
}

func v1TextConverterConvert(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output, err := c.Convert(text, option)
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	wrapResponse(context, output, nil)
}

func v1TextConverterSeg(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output := c.Seg(text, option)
	wrapResponse(context, output, nil)
}

func v1TextConverterJustConvert(context *gin.Context) {
	option, err := textConverter.CastStringToOptions(context.Param("option"))
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	text := context.Param("text")[1:]
	output, err := c.JustConvert(text, option)
	if err != nil {
		wrapResponse(context, "", err)
		return
	}
	wrapResponse(context, output, err)
}
