package textConverter

import (
	"errors"
	"strings"

	"github.com/go-ego/gse"
)

type IConverter interface {
	Init(option Options, segmenter ...*gse.Segmenter)
	seg(text string) []string
	justConvert(text string) (string, error)
	convert(text string) (string, error)
}

type Converter struct {
	s2t   IConverter
	t2s   IConverter
	s2tw  IConverter
	tw2s  IConverter
	s2hk  IConverter
	hk2s  IConverter
	s2twp IConverter
	tw2sp IConverter
	t2tw  IConverter
	t2hk  IConverter
}

type Options int

var OptionList = [...]Options{S2T, T2S, S2TW, TW2S, S2HK, HK2S, S2TWP, TW2SP, T2TW, T2HK}

const (
	S2T   Options = iota //Simplified Chinese to Traditional Chinese
	T2S                  // Traditional Chinese to Simplified Chinese
	S2TW                 // Simplified Chinese to Traditional Chinese (Taiwan Standard)
	TW2S                 // Traditional Chinese (Taiwan Standard) to Simplified Chinese
	S2HK                 // Simplified Chinese to Traditional Chinese (Hong Kong Standard)
	HK2S                 // Traditional Chinese (Hong Kong Standard) to Simplified Chinese
	S2TWP                // Simplified Chinese to Traditional Chinese (Taiwan Standard) with Taiwanese idiom
	TW2SP                // Traditional Chinese (Taiwan Standard) to Simplified Chinese with Mainland Chinese idiom
	T2TW                 // Traditional Chinese (OpenCC Standard) to Taiwan Standard
	T2HK                 // Traditional Chinese (OpenCC Standard) to Hong Kong Standard
)

func New() *Converter {
	converter := Converter{}
	converter.s2t = createConverter(S2T)
	converter.t2s = createConverter(T2S)
	converter.s2tw = createConverter(S2TW)
	converter.tw2s = createConverter(TW2S)
	converter.s2hk = createConverter(S2HK)
	converter.hk2s = createConverter(HK2S)
	converter.s2twp = createConverter(S2TWP)
	converter.tw2sp = createConverter(TW2SP)
	converter.t2tw = createConverter(T2TW)
	converter.t2hk = createConverter(T2HK)
	return &converter
}

func CastStringToOptions(str string) (Options, error) {
	switch {
	case strings.EqualFold(str, "s2t"):
		return S2T, nil
	case strings.EqualFold(str, "T2S"):
		return T2S, nil
	case strings.EqualFold(str, "S2TW"):
		return S2TW, nil
	case strings.EqualFold(str, "TW2S"):
		return TW2S, nil
	case strings.EqualFold(str, "S2HK"):
		return S2HK, nil
	case strings.EqualFold(str, "HK2S"):
		return HK2S, nil
	case strings.EqualFold(str, "S2TWP"):
		return S2TWP, nil
	case strings.EqualFold(str, "TW2SP"):
		return TW2SP, nil
	case strings.EqualFold(str, "T2TW"):
		return T2TW, nil
	case strings.EqualFold(str, "T2HK"):
		return T2HK, nil
	}
	return -1, errors.New("Not implement this option")
}

func (converter *Converter) caseConverter(option Options) *IConverter {
	switch option {
	case S2T:
		return &converter.s2t
	case T2S:
		return &converter.t2s
	case S2TW:
		return &converter.s2tw
	case TW2S:
		return &converter.tw2s
	case S2HK:
		return &converter.s2hk
	case HK2S:
		return &converter.hk2s
	case S2TWP:
		return &converter.s2twp
	case TW2SP:
		return &converter.tw2sp
	case T2TW:
		return &converter.t2tw
	case T2HK:
		return &converter.t2hk
	}
	panic("Not implement this option")
}

func (converter *Converter) Convert(text string, option Options) (string, error) {
	return (*converter.caseConverter(option)).convert(text)
}

func (converter *Converter) Seg(text string, option Options) []string {
	return (*converter.caseConverter(option)).seg(text)
}
func (converter *Converter) JustConvert(text string, option Options) (string, error) {
	return (*converter.caseConverter(option)).justConvert(text)
}
