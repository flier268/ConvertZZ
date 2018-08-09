using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using static ConvertZZ.FastReplace;

namespace ConvertZZ
{
    public class ChineseConverter
    {
        #region OS的轉換
        internal const int LOCALE_SYSTEM_DEFAULT = 0x0800;
        internal const int LCMAP_SIMPLIFIED_CHINESE = 0x02000000;
        internal const int LCMAP_TRADITIONAL_CHINESE = 0x04000000;

        /// <summary> 
        /// 使用OS的kernel.dll做為簡繁轉換工具，只要有裝OS就可以使用，不用額外引用dll，但只能做逐字轉換，無法進行詞意的轉換 
        /// <para>所以無法將電腦轉成計算機</para> 
        /// </summary> 
        [DllImport("kernel32", CharSet = CharSet.Auto, SetLastError = true)]
        internal static extern int LCMapString(int Locale, int dwMapFlags, string lpSrcStr, int cchSrc, [Out] string lpDestStr, int cchDest);

        /// <summary> 
        /// 繁體轉簡體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public static string ToSimplified(string pSource)
        {
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapString(LOCALE_SYSTEM_DEFAULT, LCMAP_SIMPLIFIED_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            return tTarget;
        }

        /// <summary> 
        /// 簡體轉繁體 
        /// </summary> 
        /// <param name="pSource">要轉換的繁體字：體</param> 
        /// <returns>轉換後的簡體字：體</returns> 
        public static string ToTraditional(string pSource)
        {
            String tTarget = new String(' ', pSource.Length);
            int tReturn = LCMapString(LOCALE_SYSTEM_DEFAULT, LCMAP_TRADITIONAL_CHINESE, pSource, pSource.Length, tTarget, pSource.Length);
            return tTarget;
        }

        #endregion OS的轉換

        private SortedDictionary<string, string> _dictionary, _dictionaryRevert;
        private bool _hasError;
        private StringBuilder _logs;
        FastReplace FR = new FastReplace(), FRRevert = new FastReplace();
        public ChineseConverter()
        {
            var cmp = new WordMappingComparer();
            _dictionary = new SortedDictionary<string, string>(cmp);
            _dictionaryRevert = new SortedDictionary<string, string>(cmp);
            _logs = new StringBuilder();
            _hasError = false;
        }
        public void ClearLogs()
        {
            _logs.Clear();
            _hasError = false;
        }
        public void Load(string fileName)
        {
            using (var reader = new StreamReader(fileName, Encoding.UTF8))
            {
                string line = reader.ReadLine();
                while (line != null)
                {
                    this.Add(line);
                    line = reader.ReadLine();
                }
            }

        }

        public void Load(string[] fileNames)
        {
            foreach (string fname in fileNames)
            {
                Load(fname);
            }
        }
        public void ReloadFastReplaceDic()
        {
            FR = new FastReplace(_dictionary);
            FRRevert = new FastReplace(_dictionaryRevert);
        }

        private void Add(string sourceWord, string targetWord)
        {
            var source = sourceWord.Split(' ');
            var target = targetWord.Split(' ');
            for (int i = 0; i < source.Length; i++)
            {
                for (int j = 0; j < target.Length; j++)
                {
                    if (j == 0)
                    {
                        if (!String.IsNullOrWhiteSpace(source[i]))
                        {
                            if (_dictionary.ContainsKey(source[i]))
                            {
                                _hasError = true;
                                _logs.AppendLine(String.Format("警告: '{0}={1}' 的來源字串重複定義, 故忽略此項。", source[i], target[j]));
                            }
                            else
                                _dictionary.Add(source[i], target[j]);
                        }
                    }
                    if (i == 0)
                    {
                        if (!String.IsNullOrWhiteSpace(target[j]))
                        {
                            if (_dictionaryRevert.ContainsKey(target[j]))
                            {
                                _hasError = true;
                                _logs.AppendLine(String.Format("警告: '{0}={1}' 的來源字串重複定義, 故忽略此項。", target[j], source[i]));
                            }
                            else
                                _dictionaryRevert.Add(target[j], source[i]);
                        }
                    }
                }
            }
        }

        public ChineseConverter Add(string mapping)
        {
            if (!String.IsNullOrWhiteSpace(mapping) && !mapping.StartsWith(";"))
            {
                Regex r = new Regex("(.*?),(.*)");
                var m = r.Match(mapping);
                if (m.Success)
                {
                    this.Add(m.Groups[1].ToString(), m.Groups[2].ToString().ToString());
                }
            }
            return this;
        }

        public ChineseConverter Add(IDictionary<string, string> dict)
        {
            foreach (var key in dict.Keys)
            {
                Add(key, dict[key]);
            }
            return this;
        }

        /// <summary>
        /// 
        /// </summary>
        /// <param name="input"></param>
        /// <param name="C2T">True:簡體轉繁體  False:繁體轉簡體</param>
        /// <returns></returns>
        public string Convert(string input, bool C2T)
        {
            //這個方法最快
            if (C2T)
                return FR.ReplaceAll(input);
            else
                return FRRevert.ReplaceAll(input);
            /* 第二快
            foreach (var temp in _dictionary)
            {
                input = input.Replace(temp.Key, temp.Value);
            }
            return input;*/
            /* 最慢
            StringBuilder sb = new StringBuilder(input);
            foreach(var temp in _dictionary)
            {
                sb.Replace(temp.Key, temp.Value);                    
            }
            return input;*/
        }

        public void DumpKeys()
        {
            foreach (var key in _dictionary.Keys)
            {
                Console.WriteLine(key);
            }
            foreach (var key in _dictionaryRevert.Keys)
            {
                Console.WriteLine(key);
            }
        }

        public bool HasError
        {
            get
            {
                return _hasError;
            }
        }

        public string Logs
        {
            get
            {
                return _logs.ToString();
            }
        }
    }
}
