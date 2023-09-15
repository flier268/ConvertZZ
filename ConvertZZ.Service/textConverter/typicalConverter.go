package textConverter

import (
	"log"
	"strings"

	"github.com/go-ego/gse"
	"github.com/rootwlg/opencc"
)

type typicalConverter struct {
	option           Options
	openCC_Converter *opencc.OpenCC
	segmenter        *gse.Segmenter
}

// Init 初始化
// option: 轉換的選項
// segmenter: 分詞器(選填，nul則會用預設分詞)
func (converter *typicalConverter) Init(option Options, segmenter ...*gse.Segmenter) {
	var err error
	switch option {
	case S2T:
		converter.openCC_Converter, err = opencc.New("s2t")
	case T2S:
		converter.openCC_Converter, err = opencc.New("t2s")
	case S2TW:
		converter.openCC_Converter, err = opencc.New("s2tw")
	case TW2S:
		converter.openCC_Converter, err = opencc.New("tw2s")
	case S2HK:
		converter.openCC_Converter, err = opencc.New("s2hk")
	case HK2S:
		converter.openCC_Converter, err = opencc.New("hk2s")
	case S2TWP:
		converter.openCC_Converter, err = opencc.New("s2twp")
	case TW2SP:
		converter.openCC_Converter, err = opencc.New("tw2sp")
	case T2TW:
		converter.openCC_Converter, err = opencc.New("t2tw")
	case T2HK:
		converter.openCC_Converter, err = opencc.New("t2hk")
	default:
		panic("Not implement this option")
	}
	converter.option = option
	if err != nil {
		log.Fatal(err)
		panic(err)
	}
	if len(segmenter) > 0 {
		converter.segmenter = segmenter[0]
	} else {
		converter.segmenter = new(gse.Segmenter)
		converter.segmenter.LoadDict()
	}
}

// seg 分詞
// text: 文字
// 返回分詞後的結果
func (converter *typicalConverter) seg(text string) []string {
	if converter.segmenter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	return converter.segmenter.Cut(text, true)
}

// justConvert 不分詞，直接轉換
// text: 文字
// 返回轉換後的結果
func (converter *typicalConverter) justConvert(text string) (string, error) {
	if converter.openCC_Converter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	return converter.openCC_Converter.Convert(text)
}

/* seg then convert */
func (converter *typicalConverter) convert(text string) (string, error) {
	if converter.segmenter == nil || converter.openCC_Converter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	builder := new(strings.Builder)
	for _, s := range converter.seg(text) {
		t, err := converter.justConvert(s)
		if err != nil {
			return "", err
		}
		builder.WriteString(t)
	}
	return builder.String(), nil
}
