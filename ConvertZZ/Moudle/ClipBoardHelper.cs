using ConvertZZ.Moudle;
using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows;
using System.Windows.Input;

namespace ConvertZZ
{
    public class ClipBoardHelper
    {
        public static string GetClipBoard_UnicodeText()
        {
            string idat = null;
            Exception threadEx = null;
            Thread staThread = new Thread(
                delegate ()
                {
                    try
                    {
                        idat = Clipboard.GetText(TextDataFormat.UnicodeText);
                    }

                    catch (Exception ex)
                    {
                        threadEx = ex;
                    }
                });
            staThread.SetApartmentState(ApartmentState.STA);
            staThread.Start();
            staThread.Join();
            return idat;
        }
        public static void SetClipBoard_UnicodeText(string str)
        {
            Exception threadEx = null;
            Thread staThread = new Thread(
                delegate ()
                {
                    try
                    {
                        Clipboard.SetText(str, TextDataFormat.UnicodeText);
                    }

                    catch (Exception ex)
                    {
                        threadEx = ex;
                    }
                });
            staThread.SetApartmentState(ApartmentState.STA);
            staThread.Start();
            staThread.Join();
        }

        internal static void Copy()
        {
            keybd_event(VK_CONTROL, 0, 0, 0);
            keybd_event(VK_C, 0, 0, 0);
            keybd_event(VK_C, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        }
        internal static void Copy(Key key, KeyModifier keyModifiers)
        {
            HotKey_KeyUp(key, keyModifiers);
            Copy();
        }
        internal static void Paste()
        {
            keybd_event(VK_CONTROL, 0, 0, 0);
            keybd_event(VK_V, 0, 0, 0);
            keybd_event(VK_V, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        }
        internal static void Paste(Key key, KeyModifier keyModifiers)
        {
            HotKey_KeyUp(key, keyModifiers);
            Paste();
        }


        private static void HotKey_KeyUp(Key key, KeyModifier keyModifiers)
        {
            System.Windows.Forms.Keys k = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(key);
            keybd_event((byte)k, 0, KEYEVENTF_KEYUP, 0);
            var array = new System.Collections.BitArray(new int[] { (int)keyModifiers });
            System.Windows.Forms.Keys m;
            if (array[0])
            {
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.LeftAlt);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.RightAlt);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
            }
            if (array[1])
            {
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.LeftCtrl);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.RightCtrl);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
            }
            if (array[2])
            {
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.LeftShift);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.RightShift);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
            }
            if (array[3])
            {
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.LWin);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
                m = (System.Windows.Forms.Keys)KeyInterop.VirtualKeyFromKey(Key.RWin);
                keybd_event((byte)m, 0, KEYEVENTF_KEYUP, 0);
            }
        }
        #region Win32
        static readonly uint KEYEVENTF_KEYUP = 2;
        static readonly byte VK_CONTROL = 0x11;
        static readonly byte VK_C = 0x43;
        static readonly byte VK_V = 0x56;

        [DllImport("user32.dll")]
        private static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, uint dwExtraInfo);
        #endregion
    }
}
